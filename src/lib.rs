use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, DirEntry};
use std::{io, env};
use std::path::Path;
use filesize::PathExt;

// define how many entries should be output
const NUM_ENTRIES: usize = 10;

/*
Config struct used to hold the string representation of the directory to recursively analyze
 */
pub struct Config {
    root_path: String,
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, &'static str> {
        if args.len() == 1 {
            // no argument passed at program invocation, default to current working directory
            let cwd = env::current_dir().unwrap_or_else(|error| {
                panic!("Unable to infer current working director, try passing a path as program argument. {:?}", error);
            });

            let path = cwd.into_os_string().into_string().unwrap_or_else(|error| {
                panic!("Unable to infer current working director, try passing a path as program argument. {:?}", error);
            });
            
            println!("No path provided as argument, defaulting to current working directory.");
            Ok(Config { root_path: String::from(path) })
        } else if args.len() > 2 {
            return Err("Expected only one argument");
        } else {
            let root_path = args[1].clone();

            Ok(Config { root_path })
        }
    }
}

/*
Results struct to hold a HashMap with file paths as keys and bytes of files as values
 */
//#[derive(Debug)]
pub struct Results {
    result_map: HashMap<String, u64>,
}

/*
Accepts Config struct as argument in order to specify the directory to analyze
*/
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    println!("Searching for {0} largest entries in {1}:\n", NUM_ENTRIES, config.root_path);

    let results = search(&config.root_path)?;

    // sort result_map by value (descending)
    let mut sort_vec: Vec<(&String, &u64)> = results.result_map.iter().collect();
    sort_vec.sort_by(|a, b| b.1.cmp(a.1));

    // Print the largest ten files found in the specified directory

    if sort_vec.len() > NUM_ENTRIES {
        for i in 0..NUM_ENTRIES {
            println!("{:?}", sort_vec[i]);
        }
    } else {
        for i in 0..sort_vec.len() {
            println!("{:?}", sort_vec[i]);
        }
    }

    Ok(())
}

/*
Converts the string representation of a directory to a file path object and an instance
of a Results struct. Passes both to visit_dirs().
 */
pub fn search(path: &str) -> Result<Results, io::Error> {
    let root_path = Path::new(path);
    let mut results = Results {
        result_map: HashMap::<String, u64>::new(),
    };

    match visit_dirs(root_path, &mut results) {
        Err(e) => eprintln!("Error calling visit_dirs() from search(): {:?}", e),
        _ => (),
    }

    Ok(results)
}

/*
Visit each file system entry in the specified directory and if it is a file, will call add_entry()
passing the entry as an argument. Otherwise, if an entry is a directory, it will be entered and searched
in a recursive manner.
 */
pub fn visit_dirs(dir: &Path, results: &mut Results) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, results)?;
            } else {
                match add_entry(&entry, results) {
                    Err(e) => eprintln!("Error calling add_entry() from visit_dirs(): {:?}", e),
                    _ => (),
                }
            }
        }
    }
    Ok(())
}

/*
For each file found, convert the file's path to a string representation and calculate the size in bytes
of the file. Insert these values into the results_map member of the Results struct.
 */
fn add_entry(entry: &DirEntry, results: &mut Results) -> io::Result<()> {
    let path = entry.path();
    let metadata = path.symlink_metadata()?;
    let size = path.size_on_disk_fast(&metadata)?;

    let str_path = path.to_str().unwrap_or("Unknown File");

    results.result_map.insert(str_path.to_string(), size);

    Ok(())
}
