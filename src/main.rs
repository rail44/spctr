mod json;
mod list;
mod map;
mod stack;
mod string;

use clap::{App, Arg};

use std::fs;
use std::io::{stdin, Read};

fn main() -> Result<(), failure::Error> {
    let matches = App::new("spctr")
        .arg(Arg::with_name("FILE").index(1))
        .arg(Arg::with_name("input").short("c").takes_value(true))
        .arg(Arg::with_name("use_stdin").short("i").takes_value(false))
        .get_matches();

    let stack = match matches.value_of("input") {
        Some(v) => stack::get_stack(v)?,
        None => {
            let path = matches.value_of("FILE").unwrap();
            let input = fs::read_to_string(path)?;
            stack::get_stack(&input)?
        }
    };

    if matches.is_present("use_stdin") {
        let mut s = String::new();
        stdin().read_to_string(&mut s)?;

        // println!(
        //     "{}",
        //     eval_source(stack, &mut Env::root())?.call(vec![Value::String(s)])?
        // );
        return Ok(());
    }
    println!(
        "{}",
        stack::eval(&stack, stack::Env::root())?.pop().unwrap()
    );
    Ok(())
}

#[test]
fn test_indexing_1() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"[1, 3][1]"#;
    assert_eq!(
        eval(&get_stack(ast).unwrap(), &mut Env::root())
            .unwrap()
            .pop()
            .unwrap(),
        Value::Number(3.0)
    );
}

#[test]
fn test_indexing_2() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"
hoge: {
    foo: "bar"
},
key: "foo",

hoge[key]"#;

    assert_eq!(
        eval(&get_stack(ast).unwrap(), &mut Env::root())
            .unwrap()
            .pop()
            .unwrap(),
        Value::String("bar".to_string())
    );
}

#[test]
fn test_call() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"
hoge: (fuga) => {
  fuga + 1
},

hoge(1)"#;
    assert_eq!(
        eval(&get_stack(ast).unwrap(), &mut Env::root())
            .unwrap()
            .pop()
            .unwrap(),
        Value::Number(2.0)
    );
}

#[test]
fn test_list() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"[1, "hoge"]"#;
    assert_eq!(
        eval(&get_stack(ast).unwrap(), &mut Env::root())
            .unwrap()
            .pop()
            .unwrap(),
        Value::List(vec![Value::Number(1.0), Value::String("hoge".to_string())])
    );
}

#[test]
fn test_getting_prop_what_its_defined() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"
fn: (prefix) => {
  hoge: prefix.concat("hoge"),
  fuga: prefix.concat("fuga")
},
obj: fn("prefix-"),

obj.hoge.concat(" ").concat(obj.fuga)"#;
    let result = eval(&get_stack(ast).unwrap(), &mut Env::root())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(result, Value::String("prefix-hoge prefix-fuga".to_string()));
}

#[test]
fn test_string_concat() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"
hoge: "hoge",
hoge.concat("fuga")"#;
    let result = eval(&get_stack(ast).unwrap(), &mut Env::root())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(result, Value::String("hogefuga".to_string()));
}

#[test]
fn test_bind_and_access() {
    use crate::stack::{eval, get_stack, Env, Value};
    let ast = r#"
hoge: {
  foo: 12,
  bar: 23,
  baz: foo + bar
},

hoge.baz
"#;
    assert_eq!(
        eval(&get_stack(ast).unwrap(), &mut Env::root())
            .unwrap()
            .pop()
            .unwrap(),
        Value::Number(35.0)
    );
}

#[test]
fn test_spread() {
    use crate::stack::{eval, get_stack, Env, Value};
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
        eval(&get_stack(ast).unwrap(), &mut Env::root())
            .unwrap()
            .pop()
            .unwrap(),
        Value::List(vec![
            Value::String("HOGE".to_string()),
            Value::String("FUGA".to_string())
        ])
    );
}
