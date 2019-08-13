mod eval;
mod list;
mod string;
mod token;
mod types;

use eval::{eval_source, Evaluable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use token::Source;
use types::Type;

#[derive(Debug, Clone, PartialEq)]
pub struct Env {
    binds: HashMap<String, token::Expression>,
    evaluated: HashMap<String, Type>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    fn get_value(&mut self, name: &str) -> Type {
        if let Some(evaluated) = self.evaluated.get(name) {
            return evaluated.clone();
        }

        if let Some(binded) = self.binds.remove(name) {
            let value = binded.eval(self);
            self.evaluated.insert(name.to_string(), value.clone());
            return value;
        }
        self.parent.as_ref().unwrap().borrow_mut().get_value(name)
    }
}

fn main() {
    let ast = "List.range({start: 1, end: 100})";

    let source = Source::from_str(ast).unwrap();
    println!("{}", eval_source(source, None));

    let ast = "fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz \"fizz\" \"\",
  buzz: if is_buzz \"buzz\" \"\",

  fizz.concat(buzz)
},
List.range({start: 1, end: 100}).map(fizzbuzz)";
    let source = Source::from_str(ast).unwrap();
    println!("{}", eval_source(source, None));

    let ast = "List";
    let source = Source::from_str(ast).unwrap();
    println!("{}", eval_source(source, None));

    let ast = "List.range({start: 0, end: 6}).map";
    let source = Source::from_str(ast).unwrap();
    println!("{}", eval_source(source, None));
}

#[test]
fn test_call() {
    let ast = "hoge: (fuga) => {
  fuga + 1
},

hoge(1)
";
    let source = Source::from_str(ast).unwrap();
    assert!(eval_source(source, None) == Type::Number(2.0));
}

#[test]
fn test_list() {
    let ast = "[1, \"hoge\"]";
    let source = Source::from_str(ast).unwrap();
    assert!(
        eval_source(source, None)
            == Type::List(vec![Type::Number(1.0), Type::String("hoge".to_string())])
    );
}

#[test]
fn test_string_concat() {
    let ast = "hoge: \"hoge\",
hoge.concat(\"fuga\")";
    let source = Source::from_str(ast).unwrap();
    assert!(eval_source(source, None) == Type::String("hogefuga".to_string()));
}

#[test]
fn test_bind_and_access() {
    let ast = "hoge: {
  foo: 12,
  bar: 23,
  baz: foo + bar
},

hoge.baz
";
    let source = Source::from_str(ast).unwrap();
    assert!(eval_source(source, None) == Type::Number(35.0));
}
