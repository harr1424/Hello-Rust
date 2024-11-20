use clap::Parser;
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// (optional) Number of largest entries to output
    #[arg(short = 'n', long = "num_entries", default_value_t = 10)]
    pub num_entries: usize,

    /// (optional) Number of files to size at one time
    #[arg(short = 'b', long = "batch_Size", default_value_t = 1000)]
    pub batch_size: usize,

    /// (optional) defaults to attempting to detect current working directory
    #[arg(short = 'd', long = "directory")]
    pub target_dir: Option<String>,

    /// (optional) Path to a file where each line specifies a directory to ignore
    #[arg(short = 'x', long = "excluded-dirs-file")]
    pub exclusion_file: Option<String>,

    #[arg(short, long)]
    pub verbose: bool,
}
