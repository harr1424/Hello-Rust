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

// TODO create a lof file in config and share it safely across threads 
// for writing errors instead of printing to the console 

/// Represents a file system entry with its path and processing result.
///
/// This struct is used during parallel directory traversal to track both
/// the path of a file and any errors encountered while processing it.
///
/// # Fields
///
/// * `path` - The file system path to the entry
/// * `result` - The processing status: `Ok(())` if no errors were encountered,
///             or `Err(SearchError)` containing the specific error
///
/// # Example
///
/// ```ignore
/// let entry = FileEntry {
///     path: PathBuf::from("/path/to/file"),
///     result: Ok(()),
/// };
///
/// // Entry with an error
/// let failed_entry = FileEntry {
///     path: PathBuf::from("/path/to/inaccessible/file"),
///     result: Err(SearchError::IoError(io::Error::new(
///         io::ErrorKind::PermissionDenied,
///         "Access denied"
///     ))),
/// };
/// ```
///
/// This struct is primarily used in batch processing operations in order to
/// track both successful and failed file operations while maintaining the original
/// file paths.
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
///
/// # Returns
///
/// Returns a tuple of `(processed, total)` where:
/// * `processed` - Number of files successfully processed and added to top_entries
/// * `total` - Total number of files attempted to process
///
/// # Error Handling
///
/// The function logs but does not propagate errors for:
/// * File metadata access failures
/// * File size calculation failures
/// * Invalid UTF-8 in path names
/// * Mutex lock failures
///
/// # Example
///
/// ```ignore
/// let batch = vec![FileEntry {
///     path: PathBuf::from("example.txt"),
///     result: Ok(()),
/// }];
/// let top_entries = Arc::new(Mutex::new(TopEntries::new(10)));
///
/// let (processed, total) = process_batch(batch, &top_entries);
/// println!("Processed {processed} out of {total} files");
/// ```
///
/// # Implementation Details
///
/// * Uses parallel iteration for metadata collection
/// * Respects previous error states of file entries - metadata collection is skipped on entry.result Err variant
/// * Maintains a thread-safe ordering of largest files
fn process_batch(batch: Vec<FileEntry>, top_entries: &Arc<Mutex<TopEntries>>) -> (usize, usize) {
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
    if !errors.is_empty() {

        // TODO write to file 
        log_errors(&errors);
    }

    (processed, total)
}

/// Performs a parallel search of files in a directory tree, sending batches of file paths to a channel.
///
/// # Arguments
///
/// * `root_dir` - The root directory to start the search from
/// * `tx` - A channel sender to transmit batches of discovered file paths
/// * `progress` - A progress bar for displaying current scanning status
/// * `skip_dirs` - Set of directory paths to exclude from the search
/// * `batch_size` - Number of file paths to collect before sending through the channel
/// * `max_open_files` - Max number of open file descriptors the program will open at one time
///
/// # Returns
///
/// Returns an `io::Result<()>` indicating whether the operation completed successfully.
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
    skip_dirs: &HashSet<String>,
    batch_size: usize,
    max_open_files: usize,
) -> Result<(), SearchError> {
    let work_queue = Arc::new(Mutex::new(VecDeque::new()));
    let is_scanning = Arc::new(AtomicBool::new(true));

    // Canonicalize skip directories with error handling
    let skip_dirs: HashSet<PathBuf> = skip_dirs
        .iter()
        .filter_map(|dir| match PathBuf::from(dir).canonicalize() {
            Ok(path) => Some(path),
            Err(err) => {
                // TODO write to file 
                let _ = log_error(&format!(
                    "Warning: Could not canonicalize skip directory '{}': {}",
                    dir, err
                ));
                None
            }
        })
        .collect();

    // Initialize work queue with root directory
    match root_dir.canonicalize() {
        Ok(root) => work_queue.lock().unwrap().push_back(root),
        Err(err) => {
            return Err(SearchError::PathError(format!(
                "Failed to canonicalize root directory: {}",
                err
            )))
        }
    }

    let mut handles = vec![];
    let open_files = Arc::new(AtomicUsize::new(0));
    let errors_count = Arc::new(AtomicUsize::new(0));

    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    for thread_id in 0..thread_count {
        let work_queue = Arc::clone(&work_queue);
        let tx = tx.clone();
        let progress = progress.clone();
        let open_files = Arc::clone(&open_files);
        let is_scanning = Arc::clone(&is_scanning);
        let skip_dirs = skip_dirs.clone();
        let errors_count = Arc::clone(&errors_count);

        handles.push(thread::spawn(move || -> Result<(), SearchError> {
            let mut batch = Vec::with_capacity(batch_size);

            'outer: loop {
                let dir = {
                    let mut queue = work_queue.lock().map_err(|e| {
                        SearchError::ThreadError(format!("Failed to lock work queue: {}", e))
                    })?;
                    queue.pop_front()
                };

                match dir {
                    Some(dir) => {
                        progress.set_message(format!(
                            "Thread {} scanning: {}",
                            thread_id,
                            dir.display()
                        ));

                        // Check if directory should be skipped
                        if let Ok(canonical_dir) = dir.canonicalize() {
                            if skip_dirs
                                .iter()
                                .any(|skip_dir| canonical_dir.starts_with(skip_dir))
                            {
                                continue;
                            }
                        }

                        // Wait for available file handle with timeout
                        let mut wait_time = 1;
                        while open_files.load(Ordering::Relaxed) >= max_open_files {
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
                                                work_queue
                                                    .lock()
                                                    .map_err(|e| {
                                                        SearchError::ThreadError(format!(
                                                            "Failed to lock work queue: {}",
                                                            e
                                                        ))
                                                    })?
                                                    .push_back(path);
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
                                    if batch.len() >= batch_size {
                                        tx.send(batch).map_err(|e| {
                                            SearchError::SendError(format!(
                                                "Failed to send batch: {}",
                                                e
                                            ))
                                        })?;
                                        batch = Vec::with_capacity(batch_size);
                                    }
                                }
                            }
                            Err(err) => {
                                errors_count.fetch_add(1, Ordering::Relaxed);
                                eprintln!("Error reading directory {}: {}", dir.display(), err);
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
            eprintln!("Thread error: {:?}", err);
        }
    }

    let error_count = errors_count.load(Ordering::Relaxed);
    progress.finish_with_message(format!(
        "Directory scan complete ({} errors encountered)",
        error_count
    ));

    Ok(())
}

/// Responsible for initiating the directory traversdal and analysis of discovered files.
///
/// # Arguments
///
/// * `config` - A `Config` struct containing:
///   * `num_entries` - Number of largest files to track and display
///   * `root_path` - Starting directory path for the search
///   * `skip_dirs` - Directories to exclude from the search
///   * `batch_size` - Number of files to process in each batch
///   * `max_open_files` - Maximum number of files to keep open simultaneously
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
///
/// # Implementation Details
///
/// - Uses a channel (`mpsc`) for communication between scanner and processor threads
/// - Maintains thread-safe access to the top entries using `Arc<Mutex<TopEntries>>`
/// - Processes files in batches for better performance
/// - Shows real-time progress using the `indicatif` crate's progress bars
/// - Writes errors to a logfile if command line arg is present
///
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
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
            &config.skip_dirs,
            config.batch_size,
            config.max_open_files,
        )
    });

    // Process files as received
    let mut total_files = 0;
    let mut total_processed = 0;
    let mut total_attempts = 0;

    while let Ok(batch) = rx.recv() {
        total_files += batch.len();
        let (processed, attempted) = process_batch(batch, &top_entries);
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
            // TODO write to file
            let _ = log_error(&format!("Scanner thread panicked: {:?}", e));
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "Scanner thread panicked. Check log file for details.".to_string(),
            )));
        }
    }

    process_progress.finish_with_message(format!(
        "Processed {} files ({} successful, {} failed)",
        total_attempts,
        total_processed,
        total_attempts - total_processed
    ));

    println!();

    // Handle top entries display with error handling
    match top_entries.lock() {
        Ok(top) => {
            if top.entries.is_empty() {
                println!("No files found matching the criteria");
            } else {
                for (path, size) in top.entries.iter() {
                    println!("{}: {}", path, size.format_size());
                }
            }
        }
        Err(e) => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to lock top entries for display: {}", e),
            )));
        }
    }

    Ok(())
}
