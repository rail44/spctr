mod eval;
mod json;
mod list;
mod map;
mod string;
mod token;
mod types;

use crate::eval::Evaluable;
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

#[derive(Debug, Clone, PartialEq)]
pub enum Unevaluated {
    Expression(token::Expression),
    Native(fn(Env) -> Result<Type, failure::Error>),
}

impl Unevaluated {
    pub fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        match self {
            Unevaluated::Expression(expression) => expression.eval(env),
            Unevaluated::Native(f) => f(env.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Env {
    bind_map: Rc<RefCell<HashMap<String, Unevaluated>>>,
    evaluated_map: Rc<RefCell<HashMap<String, Type>>>,
    parents: Vec<Env>,
}

impl Env {
    fn new(bind_map: HashMap<String, Unevaluated>, evaluated_map: HashMap<String, Type>) -> Self {
        Env {
            bind_map: Rc::new(RefCell::new(bind_map)),
            evaluated_map: Rc::new(RefCell::new(evaluated_map)),
            parents: vec![],
        }
    }

    fn root() -> Self {
        let mut evaluated_map = HashMap::new();
        evaluated_map.insert("List".to_string(), list::ListModule::get_value());
        evaluated_map.insert("Map".to_string(), map::MapModule::get_value());
        evaluated_map.insert("Json".to_string(), json::JsonModule::get_value());

        Env {
            evaluated_map: Rc::new(RefCell::new(evaluated_map)),
            ..Default::default()
        }
    }

    fn insert(&self, name: String, v: Type) {
        self.evaluated_map.borrow_mut().insert(name, v);
    }

    fn get_value(&self, name: &str) -> Result<Type, failure::Error> {
        if let Some(evaluated) = self.evaluated_map.borrow().get(name) {
            return Ok(evaluated.clone());
        }

        let binded = self.bind_map.borrow_mut().remove(name);
        if let Some(binded) = binded {
            let value = binded.eval(self)?;
            self.evaluated_map
                .borrow_mut()
                .insert(name.to_string(), value.clone());
            return Ok(value);
        }

        for p in self.parents.iter() {
            match p.get_value(name) {
                Ok(v) => return Ok(v),
                Err(_) => (),
            }
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
            eval_source(source, &mut Env::root())?.call(vec![Type::String(s)])?
        );
        return Ok(());
    }

    println!("{}", eval_source(source, &mut Env::root())?);
    Ok(())
}

#[test]
fn test_indexing_1() {
    let ast = r#"[1, 3][1]"#;
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Env::root()).unwrap(),
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
        eval_source(source, &mut Env::root()).unwrap(),
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
        eval_source(source, &mut Env::root()).unwrap(),
        Type::Number(2.0)
    );
}

#[test]
fn test_list() {
    let ast = r#"[1, "hoge"]"#;
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Env::root()).unwrap(),
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

obj.hoge.concat(" ").concat(obj.fuga)"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    assert_eq!(result, Type::String("prefix-hoge prefix-fuga".to_string()));
}

#[test]
fn test_string_concat() {
    let ast = r#"
hoge: "hoge",
hoge.concat("fuga")"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
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
        eval_source(source, &mut Env::root()).unwrap(),
        Type::Number(35.0)
    );
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
    let source = Source::from_str(ast).unwrap();
    assert_eq!(
        eval_source(source, &mut Env::root()).unwrap(),
        Type::List(vec![
            Type::String("HOGE".to_string()),
            Type::String("FUGA".to_string())
        ])
    );
}
