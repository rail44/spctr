mod ast;
mod diag;
mod eval;
mod lexer;
mod parser;
mod stdlib;

use anyhow::Result;
use clap::{App, Arg};

use std::fs;
use std::process::ExitCode;

fn main() -> Result<ExitCode> {
    let matches = App::new("spctr")
        .arg(Arg::with_name("FILE").index(1))
        .arg(Arg::with_name("input").short("c").takes_value(true))
        .arg(Arg::with_name("use_stdin").short("i").takes_value(false))
        .get_matches();

    let (filename, source) = match matches.value_of("input") {
        Some(v) => ("<inline>".to_string(), v.to_string()),
        None => {
            let path = matches.value_of("FILE").unwrap();
            (path.to_string(), fs::read_to_string(path)?)
        }
    };

    let ast = match parser::parse(&source) {
        Ok(ast) => ast,
        Err(diags) => {
            for d in &diags {
                diag::report(&filename, &source, d);
            }
            return Ok(ExitCode::FAILURE);
        }
    };

    match eval::run(&ast) {
        Ok(v) => {
            println!("{}", v);
            Ok(ExitCode::SUCCESS)
        }
        Err(d) => {
            diag::report(&filename, &source, &d);
            Ok(ExitCode::FAILURE)
        }
    }
}
