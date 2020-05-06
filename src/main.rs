mod parser;
mod token;
mod stack;
mod vm;

use clap::{App, Arg};
use anyhow::{anyhow, Result};

use std::fs;

fn main() -> Result<()> {
    let matches = App::new("spctr")
        .arg(Arg::with_name("FILE").index(1))
        .arg(Arg::with_name("input").short("c").takes_value(true))
        .arg(Arg::with_name("use_stdin").short("i").takes_value(false))
        .get_matches();

    let input = match matches.value_of("input") {
        Some(v) => {
            v.to_string()
        }
        None => {
            let path = matches.value_of("FILE").unwrap();
            fs::read_to_string(path)?
        }
    };

    eval(&input);
    Ok(())
}

fn eval(input: &str) -> Result<()> {
    let token = dbg!(parser::parse(&input)
        .map_err(|s| anyhow!("Parsing failed!, {}", s))?
        .1);
    let stack = dbg!(stack::get_cmd(&token));
    let result = vm::run(stack);
    dbg!(result);
    Ok(())
}
