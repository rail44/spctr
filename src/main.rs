mod eval;
mod list;
mod string;
mod token;
mod types;
mod json;

use eval::{eval_source, Evaluable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{stdin, BufReader, Read};
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

fn main() -> Result<(), failure::Error> {
    let mut s = String::new();
    BufReader::new(stdin()).read_to_string(&mut s)?;

    let source = Source::from_str(&s).unwrap();
    println!("{}", eval_source(source, None));
    Ok(())
}

#[test]
fn test_indexing_1() {
    let ast = r#"[1, 3][1]"#;
    let source = Source::from_str(ast).unwrap();
    assert!(eval_source(source, None) == Type::Number(3.0));
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
    assert!(eval_source(source, None) == Type::String("bar".to_string()));
}

#[test]
fn test_call() {
    let ast = r#"
hoge: (fuga) => {
  fuga + 1
},

hoge(1)"#;
    let source = Source::from_str(ast).unwrap();
    assert!(eval_source(source, None) == Type::Number(2.0));
}

#[test]
fn test_list() {
    use list::List;

    let ast = r#"[1, "hoge"]"#;
    let source = Source::from_str(ast).unwrap();
    assert!(
        eval_source(source, None)
            == Type::List(List::new(vec![
                Type::Number(1.0),
                Type::String("hoge".to_string())
            ]))
    );
}

#[test]
fn test_string_concat() {
    let ast = r#"
hoge: "hoge",
hoge.concat("fuga")"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, None);
    println!("{}", result);
    assert!(result == Type::String("hogefuga".to_string()));
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
    assert!(eval_source(source, None) == Type::Number(35.0));
}
