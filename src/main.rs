mod core;
mod formatter;
mod guesser;
mod parser;
mod types;

use parser::Parser;
use std::io::Read;

fn parse_main(data: &[u8]) -> Result<String, core::Error> {
    let mut parser = Parser::new();
    parser.parse_message(data, "root")
}

fn main() {
    let mut buffer = Vec::new();
    std::io::stdin().read_to_end(&mut buffer)
        .expect("Failed to read from stdin");
    
    match parse_main(&buffer) {
        Ok(result) => {
            println!("{}", result);
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            std::process::exit(1);
        }
    }
}
