mod parser;
mod stack;
mod token;
mod vm;

use anyhow::{anyhow, Result};
use clap::{App, Arg};

use std::fs;

fn main() -> Result<()> {
    let matches = App::new("spctr")
        .arg(Arg::with_name("FILE").index(1))
        .arg(Arg::with_name("input").short("c").takes_value(true))
        .arg(Arg::with_name("use_stdin").short("i").takes_value(false))
        .get_matches();

    let input = match matches.value_of("input") {
        Some(v) => v.to_string(),
        None => {
            let path = matches.value_of("FILE").unwrap();
            fs::read_to_string(path)?
        }
    };

    eval(&input)
}

fn eval(input: &str) -> Result<()> {
    let token = parser::parse(&input)
        .map_err(|s| anyhow!("Parsing failed!, {}", s))?
        .1;
    let cmd = stack::get_cmd(&token);
    let result = vm::run(&cmd)?;
    dbg!(result);
    Ok(())
}
