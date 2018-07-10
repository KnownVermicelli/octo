#[macro_use]
extern crate structopt;

extern crate octo_parser;

use std::io::{self, Write};
use std::string::ToString;
use structopt::StructOpt;

use octo_parser::grammar::TermParser;

#[derive(StructOpt, Debug)]
struct Parameters {
    path: Option<String>,
}

fn run_interpreter() {
    println!("Running interpreter",);
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut command_buffer = String::new();
        io::stdin().read_line(&mut command_buffer).unwrap();
        println!("Got command: {}", command_buffer.trim_right());
        interpret(command_buffer.trim_right());
    }
}

fn run_from_file(filepath: String) {
    println!("running from file: {}", filepath);
}

fn main() {
    let opt = Parameters::from_args();
    match opt.path {
        Some(path) => run_from_file(path),
        None => run_interpreter(),
    };
}

fn interpret(data: &str) -> bool {
    match parse(data) {
        Ok(something) => {
            println!("Command result: {}", something);
            true
        }
        Err(warning) => {
            println!(
                "Command failed: {} at line {}",
                warning.message, warning.line
            );
            false
        }
    }
}

struct ParsingError {
    line: u32,
    message: String,
}

fn parse(data: &str) -> Result<String, ParsingError> {
    let lexer = octo_parser::lexer::Lexer::new(data);
    let parser = TermParser::new();

    let result = parser.parse(lexer);
    println!("{:?}", result);
    if result.is_ok() {
        Result::Ok("Working".to_string())
    } else {
        Result::Err(ParsingError {
            line: 0,
            message: "error".to_string() 
        })
    }
}
