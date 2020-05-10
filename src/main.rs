mod jit;
mod parser;
mod token;

use clap::{App, Arg};

use std::fs;
use std::mem;
use crate::jit::SpctrType;
use num_traits::cast::FromPrimitive;

fn main() -> Result<(), failure::Error> {
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

    let result = eval(&input)?;
    display(result);
    Ok(())
}

fn eval(input: &str) -> Result<[u64; 32], failure::Error> {
    let token = parser::parse(&input)
        .map_err(|s| failure::format_err!("Parsing failed!, {}", s))?
        .1;
    let ptr = jit::compile(&token);
    let compiled = unsafe { mem::transmute::<_, fn() -> [u64; 32]>(ptr) };
    Ok(compiled())
}

fn display(v: [u64; 32]) {
    let kind = SpctrType::from_u64(v[0]).unwrap();
    match kind {
        SpctrType::Number => {
            println!("{}", f64::from_ne_bytes(v[1].to_ne_bytes()));
        }
        SpctrType::String => {
            println!("{}", String::from_utf8_lossy(unsafe { v[1..].align_to::<u8>().1 }));
        }
        SpctrType::Bool => {
            println!("{}", if v[1] == 0 { "false" } else { "true" });
        }
    }
}
