#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

mod parse;
mod types;

use std::fs::read;

use crate::parse::{file,E};

fn print_error(contents: &[u8], error: E) {
    for (pos, msg) in error.stuff {
        println!("Error {}", msg);
        let i = &contents[contents.len() - pos..];
        for byte in &i[..i.len().min(256)] {
            print!("{:02x} ", byte);
        }
        println!();
    }
}

fn main() {
    let matches = clap_app!(mathparse =>
        (@arg INPUT: +required "Input .vo file to parse")
        (@arg quiet: -q "Disables output messages")
        (@arg verbosity: -v +multiple "Increases message verbosity")
    ).get_matches();
    
    stderrlog::new()
        .module(module_path!())
        .quiet(matches.is_present("quiet"))
        .verbosity(matches.occurrences_of("verbosity") as usize)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    let file_name = matches.value_of("INPUT").unwrap();
    let file_contents = read(file_name).unwrap();

    match file(&file_contents) {
        Ok((_,())) => {}
        Err(nom::Err::Error(e)) => print_error(&file_contents, e),
        Err(nom::Err::Failure(e)) => print_error(&file_contents, e),
        Err(e) => panic!("{:?}", e)
    }
}
