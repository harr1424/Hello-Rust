use clap::Parser;
use ferris_files::{args::Args, config::Config, run};
use std::process;
use std::time::Instant;

fn main() {
    std::env::set_var("RUST_LOG", "trace");
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
