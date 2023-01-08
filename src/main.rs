use std::env;
use std::process;
use std::time::{Instant};

use hello_rust::Config;

fn main() {
    let start = Instant::now();

    // variable to store arguments passed at program launch
    // user should only pass in file_path to analyze a dir recursively 
    let args: Vec<String> = env::args().collect();

    // bind arguments to a Config struct
    let config = Config::build(&args).unwrap_or_else(|err| {
        eprintln!("Could not parse arguments: {err}");
        process::exit(1);
    });

    // begin program using provided configuration
    if let Err(e) = hello_rust::run(config) {
        eprintln!("Fatal Error: {e}");
        process::exit(1);
    }

    let duration = start.elapsed();
    println!("\nProgram completed in {:?} seconds", duration.as_secs_f32());
}

// program completes in about 0.4 seconds on average 
