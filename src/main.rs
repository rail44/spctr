// mod json;
// mod list;
// mod map;
mod stack;
// mod string;

use clap::{App, Arg};
use std::fs;
use std::io::{stdin, Read};

fn main() -> Result<(), failure::Error> {
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
    dbg!(stack::eval(&input)?);

    if matches.is_present("use_stdin") {
        let mut s = String::new();
        stdin().read_to_string(&mut s)?;

        // println!(
        //     "{}",
        //     eval_source(stack, &mut Env::root())?.call(vec![Type::String(s)])?
        // );
        return Ok(());
    }

    Ok(())
}

#[test]
fn test_indexing_1() {
    let ast = r#"[1, 3][1]"#;
    assert_eq!(stack::eval(ast).unwrap(), stack::Value::Number(3.0));
}

#[test]
fn test_indexing_2() {
    let ast = r#"
hoge: {
    foo: "bar"
},
key: "foo",

hoge[key]"#;

    assert_eq!(stack::eval(ast).unwrap(), stack::Value::String("bar".to_string()));
}

#[test]
fn test_call() {
    let ast = r#"
hoge: (fuga) => {
  fuga + 1
},

hoge(1)"#;
    assert_eq!(stack::eval(ast).unwrap(), stack::Value::Number(2.0));
}

#[test]
fn test_list() {
    let ast = r#"[1, "hoge"]"#;
    assert_eq!(
        stack::eval(ast).unwrap(),
        stack::Value::List(vec![stack::Value::Number(1.0), stack::Value::String("hoge".to_string())])
    );
}

#[test]
fn test_getting_prop_what_its_defined() {
    let ast = r#"
fn: (prefix) => {
  hoge: prefix.concat("hoge"),
  fuga: prefix.concat("fuga")
},
obj: fn("prefix-"),

obj.hoge.concat(" ").concat(obj.fuga)"#;
    assert_eq!(
        stack::eval(ast).unwrap(),
        stack::Value::String("prefix-hoge prefix-fuga".to_string())
    );
}

#[test]
fn test_string_concat() {
    let ast = r#"
hoge: "hoge",
hoge.concat("fuga")"#;
    assert_eq!(
        stack::eval(ast).unwrap(),
        stack::Value::String("hogefuga".to_string())
    );
}

#[test]
fn test_bind_and_access() {
    let ast = r#"
hoge: {
  foo: 12,
  bar: 23,
  baz: foo + bar
},

hoge.baz
"#;
    assert_eq!(stack::eval(ast).unwrap(), stack::Value::Number(35.0));
}

#[test]
fn test_spread() {
    let ast = r#"
map: {
  hoge: "HOGE"
},

map_2: {
    ...map,
    fuga: "FUGA"
},

[map_2.hoge, map_2.fuga]
"#;
    assert_eq!(
        stack::eval(ast).unwrap(),
        stack::Value::List(vec![
            stack::Value::String("HOGE".to_string()),
            stack::Value::String("FUGA".to_string())
        ])
    );
}
