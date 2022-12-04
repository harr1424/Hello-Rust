use std::env;
use std::process;

use hello_rust::Config;

fn main() {
    // variable to store arguments passed at program launch
    // user should only pass in file_path to analyze recursively 
    let args: Vec<String> = env::args().collect();

    // bind arguments to a Config struct
    let config = Config::build(&args).unwrap_or_else(|err| {
        eprintln!("Could not parse arguments: {err}");
        process::exit(1);
    });

    // begin program using Config
    if let Err(e) = hello_rust::run(config) {
        eprintln!("Fatal Error: {e}");
        process::exit(1);
    }
}
