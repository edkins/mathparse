#[macro_use]
extern crate clap;

mod parse;

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
    ).get_matches();
    
    let file_name = matches.value_of("INPUT").unwrap();
    let file_contents = read(file_name).unwrap();

    match file(&file_contents) {
        Ok((_,())) => {}
        Err(nom::Err::Error(e)) => print_error(&file_contents, e),
        Err(nom::Err::Failure(e)) => print_error(&file_contents, e),
        Err(e) => panic!("{:?}", e)
    }
}
