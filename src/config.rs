//! Configuration module for managing program settings and initialization.
//! 
//! This module provides functionality to build and manage program configuration,
//! including parallel processing setup, directory traversal settings, and directory
//! exclusion rules.

use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::env;
use crate::args::Args;

/// Configuration structure holding runtime settings for the program.
///
/// # Fields
///
/// * `num_entries` - Number of entries to output at program completion
/// * `batch_size` - Size of batches for processing file metadata
/// * `root_path` - Base directory path to recursively find and size files
/// * `skip_dirs` - Set of directory names to exclude from the search
///
/// # Example
///
/// ```
/// use std::error::Error;
/// use ferris_files::args::Args;
/// use ferris_files::config::Config;
/// 
/// # fn main() -> Result<(), Box<dyn Error>> {
/// let args = Args {
///     num_entries: 1000,
///     batch_size: 100,
///     target_dir: Some("./data".to_string()),
///     exclusion_file: None,
/// };
///
/// let config = Config::build(&args)?;
/// # Ok(())
/// # }
/// ```
pub struct Config {
    pub num_entries: usize,
    pub batch_size: usize,
    pub root_path: PathBuf,
    pub skip_dirs: HashSet<String>,
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
    /// 2. Sets number of entries to output equal to provided command line arg or default of 10
    /// 3. Sets batch size to match command line arg if specified or else default to 1000
    /// 4. Sets up the root directory path for operations
    /// 5. Loads directory exclusion rules if file containing dirs was supplied
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Current directory cannot be determined when no target directory is specified
    /// * Exclusion file cannot be opened or read
    /// * Thread pool configuration fails (logged as error but doesn't halt execution)
    /// If available CPU cores cannot be determined, the parallel logic will begin 
    /// using a default of one thread. See rayon documentation regarding `par_iter()` 
    /// to determine how this may impact subsequent parallel logic. 
    pub fn build(args: &Args) -> Result<Config, Box<dyn Error>> {
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .unwrap_or_else(|e| log::error!("Failed to configure thread pool: {}", e));

        println!("Program will run using {} threads", thread_count);

        let num_entries = args.num_entries;
        let batch_size = args.batch_size;

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
            num_entries,
            batch_size,
            root_path,
            skip_dirs,
        })
    }
}