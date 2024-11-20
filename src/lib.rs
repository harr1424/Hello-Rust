use filesize::PathExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::{fs, io, thread};

pub mod traits;
use crate::traits::ByteSize;

pub mod errors;
use crate::errors::SearchError;

pub mod config;
use crate::config::Config;

pub mod top_entries;
use crate::top_entries::TopEntries;

pub mod args;

pub mod tests;

/// Represents a file system entry with its path and processing result.
#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    result: Result<(), SearchError>,
}

/// Returns a platform specific (Windows or Unix) cap on open file handles.
/// On Unix will return 50% of the system's limit.
/// Windows uses a RAM based approach to allocate 64 file descriptors per 1GB of RAM.
fn get_fd_limit() -> usize {
    #[cfg(unix)]
    {
        use libc::{rlimit, RLIMIT_NOFILE};
        let mut rlim = rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // Add some debug printing
        let result = unsafe { libc::getrlimit(RLIMIT_NOFILE, &mut rlim) };
        if result == 0 {
            let limit = rlim.rlim_cur as usize;
            return limit / 2;
        } else {
            // Print the error if getrlimit fails
            println!("Error: {}", std::io::Error::last_os_error());
        }
    }

    #[cfg(windows)]
    {
        // Try to get system memory info to make an educated guess
        use windows_sys::Win32::System::SystemInformation::GetPhysicallyInstalledSystemMemory;
        let mut memory_kb: u64 = 0;
        if unsafe { GetPhysicallyInstalledSystemMemory(&mut memory_kb) } != 0 {
            let memory_gb = memory_kb / (1024 * 1024);
            // Scale based on available memory, but cap at reasonable limits
            return usize::min(usize::max(512, (memory_gb * 64) as usize), 8192);
        }
        // Fallback for Windows
        return 2048;
    }

    // Default fallback
    100
}

/// Processes a batch of file entries and updates the top_entries collection.
///
/// This function processes each file entry in parallel, collecting metadata and file sizes.
/// It handles various error conditions (IO errors, invalid paths, mutex lock failures)
/// while maintaining a count of successful and failed operations.
///
/// # Arguments
///
/// * `batch` - Vector of file entries to process. Each entry contains a path and its current processing status
/// * `top_entries` - Thread-safe collection that maintains the N largest files found so far
/// * `error_log` - Thread-safe collection that maintains a record of any errors that occurr
/// * `is_verbose` - A bool used to log error messages if true
///
/// # Returns
///
/// Returns a tuple of `(processed, total)` where:
/// * `processed` - Number of files successfully processed and added to top_entries
/// * `total` - Total number of files attempted to process
///
/// # Error Handling
///
/// The function logs errors when is_verbose is true but does not propagate errors for:
/// * File metadata access failures
/// * File size calculation failures
/// * Invalid UTF-8 in path names
/// * Mutex lock failures
///
/// # Implementation Details
///
/// * Uses parallel iteration for metadata collection
/// * Metadata collection is skipped on entry.result Err variant
/// * Maintains a thread-safe ordering of largest files
fn process_batch(
    batch: Vec<FileEntry>,
    top_entries: &Arc<Mutex<TopEntries>>,
    error_log: Arc<Mutex<Vec<String>>>,
    is_verbose: bool,
) -> (usize, usize) {
    let metadata_results: Vec<_> = batch
        .into_par_iter()
        .map(|entry| match entry.result {
            Ok(()) => match fs::metadata(&entry.path) {
                Ok(metadata) => Some((entry.path, Ok(metadata))),
                Err(err) => Some((entry.path, Err(err))),
            },
            Err(err) => Some((
                entry.path,
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Previous error: {:?}", err),
                )),
            )),
        })
        .collect();

    let total = metadata_results.len();
    let mut processed = 0;
    let mut errors = Vec::new();

    for result in metadata_results {
        if let Some((path, metadata_result)) = result {
            match metadata_result {
                Ok(metadata) => match path.size_on_disk_fast(&metadata) {
                    Ok(size) => {
                        if let Some(path_str) = path.to_str() {
                            match top_entries.lock() {
                                Ok(mut top) => {
                                    top.insert(path_str.to_string(), size);
                                    processed += 1;
                                }
                                Err(err) => {
                                    errors.push(format!(
                                        "Failed to lock top_entries for {}: {}",
                                        path.display(),
                                        err
                                    ));
                                }
                            }
                        } else {
                            errors.push(format!("Invalid UTF-8 in path: {}", path.display()));
                        }
                    }
                    Err(err) => {
                        errors.push(format!(
                            "Failed to get size for {}: {}",
                            path.display(),
                            err
                        ));
                    }
                },
                Err(err) => {
                    errors.push(format!("Error processing {}: {}", path.display(), err));
                }
            }
        }
    }

    // Log errors if any occurred
    if !errors.is_empty() && is_verbose {
        error_log.lock().unwrap().extend(errors);
    }

    (processed, total)
}

/// Performs a parallel search of files in a directory tree, sending batches of file paths to a channel.
///
/// # Arguments
///
/// * `root_dir` - The root directory to start the search from
/// * `tx` - A channel sender to transmit batches of discovered file paths
/// * `config` - Arc reference to a config instance
/// * `error_log` - Thread safe collection of errors ocurring during runtime
///
/// # Returns
///
/// Returns an `io::Result<(), SearchError>` indicating whether the operation completed successfully
/// or if a SearchError occurred
///
/// # Details
///
/// This function performs parallel directory traversal that:
/// - Uses multiple threads (based on available CPU cores) to search directories recursively
/// - Manages a shared work queue for distributing directory scanning work
/// - Limits the number of simultaneously open file handles to a platofrm specific limit or default of 100
/// - Skips symbolic links and non-existent paths
/// - Respects a set of directories to exclude from scanning
/// - Batches results to reduce channel communication overhead
///
fn parallel_search(
    root_dir: &Path,
    tx: Sender<Vec<FileEntry>>,
    progress: ProgressBar,
    config: Arc<Config>,
    error_log: Arc<Mutex<Vec<String>>>,
) -> Result<(), SearchError> {
    let work_queue = Arc::new(Mutex::new(VecDeque::new()));
    let is_scanning = Arc::new(AtomicBool::new(true));

    // Canonicalize directories to ignore
    let skip_dirs: HashSet<PathBuf> = config
        .skip_dirs
        .iter()
        .filter_map(|dir| match PathBuf::from(dir).canonicalize() {
            Ok(path) => Some(path),
            Err(err) => {
                if config.verbose {
                    error_log.lock().unwrap().push(format!(
                        "Warning: Could not canonicalize skip directory '{}': {}",
                        dir, err
                    ));
                }

                None
            }
        })
        .collect();

    // Initialize work queue with root directory
    match root_dir.canonicalize() {
        Ok(root) => work_queue.lock().unwrap().push_back(root),
        Err(err) => {
            if config.verbose {
                error_log
                    .lock()
                    .unwrap()
                    .push(format!("Failed to canonicalize root directory: {}", err));
            }
        }
    }

    let mut handles = vec![];
    let open_files = Arc::new(AtomicUsize::new(0));
    let errors_count = Arc::new(AtomicUsize::new(0));

    for _ in 0..config.num_threads {
        let work_queue = Arc::clone(&work_queue);
        let tx = tx.clone();
        let progress = progress.clone();
        let open_files = Arc::clone(&open_files);
        let is_scanning = Arc::clone(&is_scanning);
        let skip_dirs = skip_dirs.clone();
        let errors_count = Arc::clone(&errors_count);
        let config_clone = config.clone();
        let error_log = error_log.clone();

        handles.push(thread::spawn(move || -> Result<(), SearchError> {
            let mut batch = Vec::with_capacity(config_clone.batch_size);

            'outer: loop {
                let dir = {
                    match work_queue.lock() {
                        Ok( mut q) => {
                            q.pop_front()
                        }
                        Err(e) => {
                            if config_clone.verbose {
                                error_log
                                    .lock()
                                    .unwrap()
                                    .push(format!("Failed to lock work queue: {}", e));
                            }
                            None
                        }
                    }
                };

                match dir {
                    Some(dir) => {
                        progress.set_message(format!("Scanning: {}", dir.display()));

                        // Check if directory should be skipped
                        match dir.canonicalize() {
                            Ok(canonical_dir) => {
                                if skip_dirs
                                    .iter()
                                    .any(|skip_dir| canonical_dir.starts_with(skip_dir))
                                {
                                    continue;
                                }
                            }
                            Err(e) => {
                                if config_clone.verbose {
                                    error_log.lock().unwrap().push(format!("Failed to canonicalize directory {:#?} : {}", dir, e));
                                }
                            }
                        }

                        // Wait for available file handle with timeout
                        let mut wait_time = 1;
                        while open_files.load(Ordering::Relaxed) >= config_clone.max_open_files {
                            thread::sleep(std::time::Duration::from_millis(wait_time));
                            wait_time = wait_time.saturating_mul(2).min(100); // Exponential backoff
                        }
                        open_files.fetch_add(1, Ordering::SeqCst);

                        match fs::read_dir(&dir) {
                            Ok(entries) => {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.is_symlink() {
                                        continue;
                                    }

                                    let file_entry = match path.metadata() {
                                        Ok(metadata) => {
                                            if metadata.is_dir() {
                                                match work_queue.lock() {
                                                    Ok(mut q) => {
                                                        q.push_back(path);
                                                    }
                                                    Err(e) => {
                                                        if config_clone.verbose {
                                                            error_log.lock().unwrap().push(format!("Error obtaining lock on work queue: {}", e));
                                                        }
                                                    }
                                                }
                                                continue;
                                            }
                                            FileEntry {
                                                path,
                                                result: Ok(()),
                                            }
                                        }
                                        Err(err) => {
                                            errors_count.fetch_add(1, Ordering::Relaxed);
                                            FileEntry {
                                                path,
                                                result: Err(SearchError::IoError(err)),
                                            }
                                        }
                                    };

                                    batch.push(file_entry);
                                    if batch.len() >= config_clone.batch_size {
                                        tx.send(batch).map_err(|e| {
                                            SearchError::SendError(format!(
                                                "Failed to send batch: {}",
                                                e
                                            ))
                                        })?;
                                        batch = Vec::with_capacity(config_clone.batch_size);
                                    }
                                }
                            }
                            Err(err) => {
                                errors_count.fetch_add(1, Ordering::Relaxed);
                                if config_clone.verbose {
                                    error_log.lock().unwrap().push(format!("Error reading directory {}: {}", dir.display(), err));
                                }
                            }
                        }

                        open_files.fetch_sub(1, Ordering::SeqCst);
                    }
                    None => {
                        if !is_scanning.load(Ordering::SeqCst) {
                            if work_queue
                                .lock()
                                .map_err(|e| {
                                    SearchError::ThreadError(format!(
                                        "Failed to lock work queue: {}",
                                        e
                                    ))
                                })?
                                .is_empty()
                            {
                                break 'outer;
                            }
                        }
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }

            if !batch.is_empty() {
                tx.send(batch).map_err(|e| {
                    SearchError::SendError(format!("Failed to send final batch: {}", e))
                })?;
            }

            Ok(())
        }));
    }

    is_scanning.store(false, Ordering::SeqCst);

    // Join threads and collect errors
    let thread_results: Vec<Result<(), SearchError>> = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .map_err(|e| SearchError::ThreadError(format!("Thread panicked: {:?}", e)))?
        })
        .collect();

    // Check for any thread errors
    for result in thread_results {
        if let Err(err) = result {
            if config.verbose {
                error_log
                    .lock()
                    .unwrap()
                    .push(format!("Thread error: {:?}", err));
            }
        }
    }

    let error_count = errors_count.load(Ordering::Relaxed);
    progress.finish_with_message(format!(
        "Directory scan complete ({} errors encountered)",
        error_count
    ));

    Ok(())
}

/// Responsible for initiating the directory traversdal and analyzing files as they are discovered
///
/// # Arguments
///
/// * `config` - An instance of a `Config` struct
///
/// # Returns
///
/// * `Result<(), Box<dyn Error>>` - Ok(()) if successful, or an Error if something fails
///
/// # Progress Display
///
/// The function shows two progress indicators:
/// 1. A spinner showing the directory scanning progress
/// 2. A spinner showing file processing progress with counts of total and successfully processed files
///
/// # Output
///
/// Upon completion, prints a list of the largest files found, with their paths and sizes.
/// If verbsoity was enabled, errors will be printed before file size results.
///
/// # Implementation Details
///
/// - Uses a channel (`mpsc`) for communication between scanner and processor threads
/// - Maintains thread-safe access to the top entries using `Arc<Mutex<TopEntries>>`
/// - Processes files in batches for better performance
/// - Shows real-time progress using the `indicatif` crate's progress bars
///
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let is_verbose = config.verbose;
    let error_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let error_log_clone = error_log.clone();
    let config_arc: Arc<Config> = Arc::new(config.clone());

    print!(
        "Searching for {0} largest entries in {1}:\n",
        config.num_entries,
        config.root_path.display()
    );

    let multi_progress = MultiProgress::new();
    let scan_progress = multi_progress.add(ProgressBar::new_spinner());
    scan_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );

    let process_progress = multi_progress.add(ProgressBar::new_spinner());
    process_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );

    let (tx, rx) = mpsc::channel();
    let top_entries = Arc::new(Mutex::new(TopEntries::new(config.num_entries)));

    // Directory scanner thread
    let root_path = config.root_path.clone();
    let scan_handle = thread::spawn(move || {
        parallel_search(
            &root_path,
            tx,
            scan_progress,
            config_arc.clone(),
            error_log_clone.clone(),
        )
    });

    // Process files as received
    let mut total_files = 0;
    let mut total_processed = 0;
    let mut total_attempts = 0;

    while let Ok(batch) = rx.recv() {
        total_files += batch.len();
        let (processed, attempted) =
            process_batch(batch, &top_entries, error_log.clone(), is_verbose);
        total_processed += processed;
        total_attempts += attempted;

        process_progress.set_message(format!(
            "Processing {} files (successfully processed: {}, failed: {})...",
            total_files,
            total_processed,
            total_attempts - total_processed
        ));
    }

    // Handle scanner thread result
    match scan_handle.join() {
        Ok(result) => result.map_err(|e| Box::new(e))?,
        Err(e) => {
            if is_verbose {
                error_log
                    .lock()
                    .unwrap()
                    .push(format!("Scanner thread panicked: {:?}", e));
            }
        }
    }

    process_progress.finish_with_message(format!(
        "Processed {} files ({} successful, {} failed)",
        total_attempts,
        total_processed,
        total_attempts - total_processed
    ));

    if is_verbose {
        println!();
        error_log.lock().unwrap().iter().for_each(|e| {
            eprintln!("{}", e);
        });
    }

    println!();

    match top_entries.lock() {
        Ok(top) => {
            if top.entries.is_empty() {
                println!("No files found - run with -v flag for error output");
            } else {
                for (path, size) in top.entries.iter() {
                    println!("{}: {}", path, size.format_size());
                }
            }
        }
        Err(e) => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to lock top entries for final output: {}", e),
            )));
        }
    }

    Ok(())
}
