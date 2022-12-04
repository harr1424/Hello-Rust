use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, DirEntry};
use std::io;
use std::path::{self, Path};

use filesize::PathExt;

/*
Accepts Config struct as argument in order to specify
search string and the file in which to search for the string.
*/
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub struct Config {
    root_path: String,
}

pub struct Results {
    results: HashMap<String, u32>,
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 2 {
            return Err("File path was not specified");
        } else if args.len() > 2 {
            return Err("Usage: disk_storage /User/Documents/");
        } else {
            let root_path = args[1].clone();

            Ok(Config { root_path })
        }
    }
}

pub fn search(path: &str) -> io::Result<()> {
    let root_path = Path::new(path);

    visit_dirs(root_path, &add_entry);

    Ok(())
}

pub fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}

fn add_entry(entry: &DirEntry) {
    let path = entry.path();
    let metadata = path.symlink_metadata()?;
    let size = path.size_on_disk_fast(&metadata);
}
