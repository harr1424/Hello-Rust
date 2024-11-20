use crate::args::Args;
use crate::get_fd_limit;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Configuration structure containing runtime settings.
///
/// # Fields
///
/// * `num_threads` - Number of threads to use in parallel processing
/// * `num_entries` - Number of entries to output at program completion
/// * `batch_size` - Size of batches for processing file metadata
/// * `root_path` - Base directory path to recursively find and size files
/// * `skip_dirs` - Set of directory names to exclude from the search
/// * `max_open_files` - Maximum number of open file handles used by this program
/// * `verbose` - Bool to determine if errors collected during runtime will be printed
///
#[derive(Clone)]
pub struct Config {
    pub num_threads: usize,
    pub num_entries: usize,
    pub batch_size: usize,
    pub root_path: PathBuf,
    pub skip_dirs: HashSet<String>,
    pub max_open_files: usize,
    pub verbose: bool,
}

impl Config {
    /// Builds a new Config instance from provided command line arguments.
    ///
    /// # Parameters
    ///
    /// * `args` - Reference to Args structure containing command line arguments
    ///
    /// # Returns
    ///
    /// * `Result<Config, Box<dyn Error>>` - New Config instance or error if construction fails
    ///
    /// # Details
    ///
    /// This function performs the following setup:
    /// 1. Configures parallel processing based on available CPU cores
    /// 2. Calls a library function to determine platform specific cap on open file descriptors
    /// 3. Sets number of entries to output equal to provided command line arg or default of 10
    /// 4. Sets batch size to match command line arg if specified or else default to 1000
    /// 5. Sets verbose bool to match command line arg
    /// 6. Sets up the root directory path for operations
    /// 7. Loads directory exclusion rules if file containing dirs was supplied
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Current directory cannot be determined when no target directory is specified
    /// * Exclusion file cannot be opened or read
    /// * Thread pool configuration fails (logged as error but doesn't halt execution)
    /// 
    pub fn build(args: &Args) -> Result<Config, Box<dyn Error>> {
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        println!("Preparing to scan using {} threads", num_threads);

        let max_open_files = get_fd_limit();
        println!("Limiting open file handles to {}", max_open_files);

        let num_entries = args.num_entries;
        let batch_size = args.batch_size;
        let verbose = args.verbose;

        let root_path = if let Some(target_dir) = &args.target_dir {
            PathBuf::from(target_dir)
        } else {
            env::current_dir()?
        };

        let mut skip_dirs: HashSet<String> = HashSet::new();
        if let Some(exclusion_file) = &args.exclusion_file {
            let file = File::open(exclusion_file)
                .expect("A path to an excluded directories file was provided but the file could not be read");

            let reader = BufReader::new(file);
            reader.lines().for_each(|line| match line {
                Ok(dir) => {
                    skip_dirs.insert(dir);
                }
                Err(e) => log::error!("Error reading line: {}", e),
            });
        }

        Ok(Config {
            num_threads,
            num_entries,
            batch_size,
            root_path,
            skip_dirs,
            max_open_files,
            verbose
        })
    }
}
