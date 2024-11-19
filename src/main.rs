use std::time::Instant;
use std::process;
use clap::Parser;
use ferris_files::{args::Args, config::Config, run};

// TODO explore adding support for finding largest dirs instead of files
// if pursued use a flag to control either files or dirs
fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let start = Instant::now();

    let args = Args::parse();

    let config = Config::build(&args).unwrap_or_else(|err| {
        log::error!("Could not parse arguments: {}", err);
        process::exit(1);
    });

    if let Err(e) = run(config) {
        log::error!("Fatal Error: {e}");
        process::exit(1);
    }

    let duration = start.elapsed();
    println!(
        "\nProgram completed in {:?} seconds",
        duration.as_secs_f32()
    );
}
