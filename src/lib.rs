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

pub mod config;
use crate::config::Config;

pub mod top_entries;
use crate::top_entries::TopEntries;

pub mod args;

pub mod tests;


// Add doc comments
// used to obtain metadata (file size) on a batch of files
// peerformed in batches to improve performance
fn process_batch(batch: Vec<PathBuf>, top_entries: &Arc<Mutex<TopEntries>>) -> usize {
    let metadata_results: Vec<_> = batch
        .par_iter()
        .map(|path| fs::metadata(path).ok().map(|m| (path, m)))
        .collect();

    let mut processed = 0;
    for result in metadata_results.iter().flatten() {
        let (path, metadata) = result;
        if let Ok(size) = path.size_on_disk_fast(metadata) {
            if let Some(path_str) = path.to_str() {
                let mut top = top_entries.lock().unwrap();
                top.insert(path_str.to_string(), size);
                processed += 1;
            }
        }
    }

    processed
}

fn parallel_search(
    root_dir: &Path,
    tx: Sender<Vec<PathBuf>>,
    progress: ProgressBar,
    skip_dirs: &HashSet<String>,
    batch_size: usize,
) -> io::Result<()> {
    let work_queue = Arc::new(Mutex::new(VecDeque::new()));
    let is_scanning = Arc::new(AtomicBool::new(true));

    let skip_dirs: HashSet<PathBuf> = skip_dirs
        .iter()
        .filter_map(|dir| PathBuf::from(dir).canonicalize().ok())
        .collect();

    work_queue.lock().unwrap().push_back(root_dir.to_path_buf());

    let mut handles = vec![];
    let open_files = Arc::new(AtomicUsize::new(0));

    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    for _ in 0..thread_count {
        let work_queue = Arc::clone(&work_queue);
        let tx = tx.clone();
        let progress = progress.clone();
        let open_files = Arc::clone(&open_files);
        let is_scanning = Arc::clone(&is_scanning);
        let skip_dirs = skip_dirs.clone();

        handles.push(thread::spawn(move || {
            let mut batch = Vec::with_capacity(batch_size);

            'outer: loop {
                // Get next directory to process
                let dir = {
                    let mut queue = work_queue.lock().unwrap();
                    queue.pop_front()
                };

                match dir {
                    Some(dir) => {
                        progress.set_message(format!("Scanning: {}", dir.display()));

                        if let Ok(canonical_dir) = dir.canonicalize() {
                            if skip_dirs
                                .iter()
                                .any(|skip_dir| canonical_dir.starts_with(skip_dir))
                            {
                                continue;
                            }
                        }

                        // Wait for available file handle
                        loop {
                            let current = open_files.load(Ordering::Relaxed);
                            if current >= 100 {
                                thread::sleep(std::time::Duration::from_millis(1));
                                continue;
                            }
                            if open_files
                                .compare_exchange(
                                    current,
                                    current + 1,
                                    Ordering::SeqCst,
                                    Ordering::SeqCst,
                                )
                                .is_ok()
                            {
                                break;
                            }
                        }

                        if let Ok(entries) = fs::read_dir(&dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if path.is_symlink() {
                                    continue;
                                }
                                if path.exists() {
                                    if path.is_dir() {
                                        work_queue.lock().unwrap().push_back(path);
                                    } else {
                                        batch.push(path);
                                        if batch.len() >= batch_size {
                                            tx.send(batch).unwrap();
                                            batch = Vec::with_capacity(batch_size);
                                        }
                                    }
                                }
                            }
                        }

                        open_files.fetch_sub(1, Ordering::SeqCst);
                    }
                    None => {
                        // No work available - check if we should exit
                        if !is_scanning.load(Ordering::SeqCst) {
                            // Double-check work queue is empty under lock
                            if work_queue.lock().unwrap().is_empty() {
                                break 'outer;
                            }
                        }
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }

            // Send any remaining files in the final batch
            if !batch.is_empty() {
                tx.send(batch).unwrap();
            }
        }));
    }

    // Signal threads to finish before joining
    is_scanning.store(false, Ordering::SeqCst);
    for handle in handles {
        handle.join().unwrap();
    }

    progress.finish_with_message("Directory scan complete");
    Ok(())
}

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

    //  Directory scanner thread
    let root_path = config.root_path.clone();
    let scan_handle = thread::spawn(move || {
        parallel_search(&root_path, tx, scan_progress, &config.skip_dirs, config.batch_size)
    });

    // Process files as received
    let mut total_files = 0;
    let mut processed_files = 0;

    while let Ok(batch) = rx.recv() {
        total_files += batch.len();
        process_progress.set_message(format!(
            "Processing {} files (successfully processed: {})...",
            total_files, processed_files
        ));

        processed_files += process_batch(batch, &top_entries);
    }

    scan_handle.join().unwrap()?;
    process_progress.finish_with_message(format!(
        "Processed {} files ({} successful)",
        total_files, processed_files
    ));

    println!();
    let top = top_entries.lock().unwrap();
    for (path, size) in top.entries.iter() {
        println!("{}: {}", path, size.format_size());
    }

    Ok(())
}
