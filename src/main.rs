use filesize::PathExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use std::{env, io};

const NUM_ENTRIES: usize = 10;
const BATCH_SIZE: usize = 10000;

// TODO allow users to supply a file containing excluded dirs
const SKIP_DIRS: &[&str] = &[
    //"/Users/user/Library/Developer/CoreSimulator/Devices",
    //"/Users/user/Library/Android/sdk/system-images/",
    // "node_modules",
    // "target",
    // ".git",
    // "dist",
    // "build",
    // ".next",
    // ".venv",
    // "venv",
];

trait ByteSize {
    fn format_size(&self) -> String;
}

impl ByteSize for u64 {
    fn format_size(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        match self {
            bytes if *bytes >= TB => format!("{:.2} TB", *bytes as f64 / TB as f64),
            bytes if *bytes >= GB => format!("{:.2} GB", *bytes as f64 / GB as f64),
            bytes if *bytes >= MB => format!("{:.2} MB", *bytes as f64 / MB as f64),
            bytes if *bytes >= KB => format!("{:.2} KB", *bytes as f64 / KB as f64),
            bytes => format!("{} bytes", bytes),
        }
    }
}

pub struct Config {
    root_path: PathBuf,
    skip_dirs: HashSet<String>,
}

// TODO use logging instead of print statements
impl Config {
    pub fn build(args: &[String]) -> Result<Config, Box<dyn Error>> {
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        
        rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .unwrap_or_else(|e| eprintln!("Failed to configure thread pool: {}", e));

        println!("Program will run using {} threads", thread_count);

        let root_path = if args.len() == 1 {
            println!("No path provided as argument, defaulting to current working directory.");
            env::current_dir()
                .map_err(|e| format!("Unable to infer current working directory: {}", e))?
        } else if args.len() > 2 {
            return Err("Expected only one argument".into());
        } else {
            PathBuf::from(&args[1])
        };

        let skip_dirs = SKIP_DIRS.iter().map(|&s| s.to_string()).collect();

        Ok(Config {
            root_path,
            skip_dirs,
        })
    }
}

#[derive(Debug)]
struct TopEntries {
    entries: Vec<(String, u64)>,
    max_entries: usize,
}

impl TopEntries {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries + 1),
            max_entries,
        }
    }

    // TODO this should probably have some doc tests
    fn insert(&mut self, path: String, size: u64) {
        if self.entries.len() < self.max_entries
            || size > self.entries.last().map(|(_, s)| *s).unwrap_or(0)
        {
            let idx = self.entries.partition_point(|(_, s)| *s > size);
            self.entries.insert(idx, (path, size));

            if self.entries.len() > self.max_entries {
                self.entries.pop();
            }
        }
    }

    // Used only for test coverage
    #[allow(dead_code)]
    fn get_entries(&self) -> &[(String, u64)] {
        &self.entries
    }
}

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

fn parallel_collect_entries(
    root_dir: &Path,
    tx: Sender<Vec<PathBuf>>,
    progress: ProgressBar,
    skip_dirs: &HashSet<String>,
) -> io::Result<()> {
    let work_queue = Arc::new(Mutex::new(VecDeque::new()));
    let is_scanning = Arc::new(AtomicBool::new(true));

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
            let mut batch = Vec::with_capacity(BATCH_SIZE);

            'outer: loop {
                // Get next directory to process
                let dir = {
                    let mut queue = work_queue.lock().unwrap();
                    queue.pop_front()
                };

                match dir {
                    Some(dir) => {
                        progress.set_message(format!("Scanning: {}", dir.display()));

                        if skip_dirs
                            .iter()
                            .any(|skip_dir| dir.to_string_lossy().contains(skip_dir))
                        {
                            continue;
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
                                        if skip_dirs.iter().any(|skip_dir| {
                                            path.to_string_lossy().contains(skip_dir)
                                        }) {
                                            continue;
                                        }
                                        work_queue.lock().unwrap().push_back(path);
                                    } else {
                                        batch.push(path);
                                        if batch.len() >= BATCH_SIZE {
                                            tx.send(batch).unwrap();
                                            batch = Vec::with_capacity(BATCH_SIZE);
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
    println!(
        "Searching for {0} largest entries in {1}:\n",
        NUM_ENTRIES,
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
    let top_entries = Arc::new(Mutex::new(TopEntries::new(NUM_ENTRIES)));

    //  Directory scanner thread
    let root_path = config.root_path.clone();
    let scan_handle = thread::spawn(move || {
        parallel_collect_entries(&root_path, tx, scan_progress, &config.skip_dirs)
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

    let top = top_entries.lock().unwrap();
    for (path, size) in top.entries.iter() {
        println!("{}: {}", path, size.format_size());
    }

    Ok(())
}

// TODO Use clap to allow the following command line args: 
// hidden file flag
// path to ignored dirs 

// TODO explore adding support for finding largest dirs instead of files
// if pursued use a flag to control either files or dirs 
fn main() {
    let start = Instant::now();

    let args: Vec<String> = env::args().collect();

    let config = Config::build(&args).unwrap_or_else(|err| {
        eprintln!("Could not parse arguments: {err}");
        process::exit(1);
    });

    if let Err(e) = run(config) {
        eprintln!("Fatal Error: {e}");
        process::exit(1);
    }

    let duration = start.elapsed();
    println!(
        "\nProgram completed in {:?} seconds",
        duration.as_secs_f32()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_entries_ordering() {
        let mut top = TopEntries::new(3);

        top.insert("medium".to_string(), 50);
        top.insert("largest".to_string(), 100);
        top.insert("smallest".to_string(), 25);

        let entries = top.get_entries();
        assert_eq!(entries[0].1, 100); // Largest first
        assert_eq!(entries[1].1, 50); // Medium second
        assert_eq!(entries[2].1, 25); // Smallest last
    }

    #[test]
    fn test_top_entries_capacity() {
        let mut top = TopEntries::new(2);

        top.insert("large".to_string(), 100);
        top.insert("medium".to_string(), 50);
        top.insert("small".to_string(), 25); // Should be dropped as it's smallest

        let entries = top.get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, 100); // Largest kept
        assert_eq!(entries[1].1, 50); // Medium kept
    }

    #[test]
    fn test_top_entries_replacement() {
        let mut top = TopEntries::new(2);

        top.insert("medium".to_string(), 50);
        top.insert("small".to_string(), 25);
        top.insert("large".to_string(), 100); // Should push out smallest

        let entries = top.get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, 100); // Largest first
        assert_eq!(entries[1].1, 50); // Medium second
                                      // 25 was dropped as it was smallest
    }
}
