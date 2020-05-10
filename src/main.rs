mod jit;
mod parser;
mod token;

use clap::{App, Arg};

use std::fs;
use std::mem;

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
    println!("{}", result.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "));
    println!("{}", f64::from_ne_bytes(result[0].to_ne_bytes()));
    println!("{}", String::from_utf8_lossy(unsafe { result.align_to::<u8>().1 }));
    Ok(())
}

fn eval(input: &str) -> Result<[u64; 64], failure::Error> {
    let token = parser::parse(&input)
        .map_err(|s| failure::format_err!("Parsing failed!, {}", s))?
        .1;
    println!("{:?}", token);
    let ptr = jit::compile(&token);
    let compiled = unsafe { mem::transmute::<_, fn() -> [u64; 64]>(ptr) };
    Ok(compiled())
}
