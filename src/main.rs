mod eval;
mod json;
mod list;
mod map;
mod string;
mod token;
mod types;

use clap::{App, Arg};
use eval::eval_source;
use failure::format_err;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{stdin, Read};
use std::rc::Rc;
use std::str::FromStr;
use token::Source;
use types::Type;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Env {
    binds: HashMap<String, Type>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    fn get_value(&mut self, name: &str) -> Result<Type, failure::Error> {
        if let Some(binded) = self.binds.remove(name) {
            let value = binded.eval(self)?;
            self.binds.insert(name.to_string(), value.clone());
            return Ok(value);
        }

        if let Some(p) = self.parent.as_ref() {
            return p.borrow_mut().get_value(name);
        }

        Err(format_err!("Could not find bind `{}`", name))
    }
}

fn main() -> Result<(), failure::Error> {
    let matches = App::new("spctr")
        .arg(Arg::with_name("FILE").index(1))
        .arg(Arg::with_name("input").short("c").takes_value(true))
        .arg(Arg::with_name("use_stdin").short("i").takes_value(false))
        .get_matches();

    let source = match matches.value_of("input") {
        Some(v) => Source::from_str(v).unwrap(),
        None => {
            let path = matches.value_of("FILE").unwrap();
            let input = fs::read_to_string(path)?;
            Source::from_str(&input).unwrap()
        }
    };

    if matches.is_present("use_stdin") {
        let mut s = String::new();
        stdin().read_to_string(&mut s)?;

        println!(
            "{}",
            eval_source(source, &mut Default::default())?.call(vec![Type::String(s)])?
        );
        return Ok(());
    }

    println!("{}", eval_source(source, &mut Default::default())?);
    Ok(())
}

#[test]
fn test_indexing_1() {
    let ast = r#"[1, 3][1]"#;
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Default::default()).unwrap(),
        Type::Number(3.0)
    );
}

#[test]
fn test_indexing_2() {
    let ast = r#"
hoge: {
    foo: "bar"
},
key: "foo",

hoge[key]"#;

    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Default::default()).unwrap(),
        Type::String("bar".to_string())
    );
}

#[test]
fn test_call() {
    let ast = r#"
hoge: (fuga) => {
  fuga + 1
},

hoge(1)"#;
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Default::default()).unwrap(),
        Type::Number(2.0)
    );
}

#[test]
fn test_list() {
    let ast = r#"[1, "hoge"]"#;
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Default::default()).unwrap(),
        Type::List(vec![Type::Number(1.0), Type::String("hoge".to_string())])
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

obj.hoge.concat(" ", obj.fuga)"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
    assert_eq!(result, Type::String("prefix-hoge prefix-fuga".to_string()));
}

#[test]
fn test_string_concat() {
    let ast = r#"
hoge: "hoge",
hoge.concat("fuga")"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
    assert_eq!(result, Type::String("hogefuga".to_string()));
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
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Default::default()).unwrap(),
        Type::Number(35.0)
    );
}
