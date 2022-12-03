use std::env;
use std::process;

use my_grep::Config;

fn main() {
    // variable to store arguments passed at program launch
    let args: Vec<String> = env::args().collect();

    // bind arguments to a Config struct
    let config = Config::build(&args).unwrap_or_else(|err| {
        println!("Could not parse arguments: {err}");
        process::exit(1);
    });

    // begin program using Config
    if let Err(e) = my_grep::run(config) {
        println!("Fatal Error: {e}");
        process::exit(1);
    }
}
