use std::fs;
use std::error::Error;


/*
Accepts Config struct as argument in order to specify 
search string and the file in which to search for the string. 
*/
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    println!(
        "Searching for: '{0}' using file path: {1}:",
        config.query, config.file_path
    );

    let contents = fs::read_to_string(config.file_path)?;

    println!("Found:\n{contents}");

    Ok(())
}

pub struct Config {
    query: String,
    file_path: String,
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 3 {
            return Err("Not enough arguments provided");
        }

        let query = args[1].clone();
        let file_path = args[2].clone();

        Ok(Config { query, file_path })
    }
}