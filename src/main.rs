mod parser;
mod jit;
mod token;

use clap::{App, Arg};

use std::fs;
use std::io::{stdin, Read};
use std::mem;

fn main() -> Result<(), failure::Error> {
    let matches = App::new("spctr")
        .arg(Arg::with_name("FILE").index(1))
        .arg(Arg::with_name("input").short("c").takes_value(true))
        .arg(Arg::with_name("use_stdin").short("i").takes_value(false))
        .get_matches();

    let token = match matches.value_of("input") {
        Some(v) => parser::parse(v).map_err(|s| failure::format_err!("Parsing failed!, {}", s))?.1,
        None => {
            let path = matches.value_of("FILE").unwrap();
            let input = fs::read_to_string(path)?.clone();
            parser::parse(&input).map_err(|s| failure::format_err!("Parsing failed!, {}", s))?.1
        }
    };

    if matches.is_present("use_stdin") {
        let mut s = String::new();
        stdin().read_to_string(&mut s)?;

        // println!(
        //     "{}",
        //     eval_source(token, &mut Env::root())?.call(vec![Value::String(s)])?
        // );
        return Ok(());
    }

    println!(
        "{:?}",
        token
    );

    let ptr = jit::compile(&token);
    let compiled = unsafe { mem::transmute::<_, fn() -> f64>(ptr) };
    println!(
        "{:?}",
        compiled()
    );
    Ok(())
}
