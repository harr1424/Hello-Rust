use clap::Parser;
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Number of largest entries to output (defaults to 10)
    #[arg(short = 'n', long = "num_entries", default_value_t = 10)]
    pub num_entries: usize,

    /// Number of files to calculate size of at one time (defaults to 1000)
    #[arg(short = 'b', long = "batch_Size", default_value_t = 1000)]
    pub batch_size: usize,

    /// Directory path (optional - defaults to current working directory)
    #[arg(short = 'd', long = "directory")]
    pub target_dir: Option<String>,

    /// Excluded directories filepath (optional)
    #[arg(short = 'x', long = "excluded-dirs-file")]
    pub exclusion_file: Option<String>,
}
