mod array;
mod eval;
mod string;
mod token;

use eval::{eval_source, Evaluable};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::IntoIterator;
use std::rc::Rc;
use std::str::FromStr;
use token::Source;

#[derive(Debug, Clone)]
pub struct NativeType(Rc<dyn Native>);

impl NativeType {
    fn new<N: Native>(v: N) -> Self {
        NativeType(Rc::new(v))
    }
}

pub trait Native: 'static + Debug {
    fn get_prop(&self, env: &mut Env, name: &str) -> Type;
    fn call(&self, env: &mut Env, args: Vec<Type>) -> Type;
    fn comparator(&self) -> &str;
}

impl<N: Native> From<N> for Type {
    fn from(n: N) -> Self {
        Type::Native(NativeType::new(n))
    }
}

impl PartialEq for NativeType {
    fn eq(&self, other: &Self) -> bool {
        self.0.type_id() == other.0.type_id() && self.0.comparator() == other.0.comparator()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    Array(Vec<Type>),
    Map(HashMap<String, token::Expression>),
    Function(Env, Vec<String>, Box<token::Expression>),
    Boolean(bool),
    Native(NativeType),
}

impl Type {
    fn get_prop(&self, env: &mut Env, name: &str) -> Type {
        match self {
            Type::Map(map) => {
                let mut child = Env {
                    binds: map.clone(),
                    evaluated: HashMap::new(),
                    parent: Some(Rc::new(RefCell::new(env.clone()))),
                };
                child.get_value(name)
            }
            Type::Array(v) => match name {
                "map" => array::Map::new(v.clone()).into(),
                _ => panic!(),
            },
            Type::String(s) => match name {
                "concat" => string::Concat::new(s.clone()).into(),
                _ => panic!(),
            },
            Type::Native(n) => n.0.get_prop(env, name),
            _ => unreachable!(),
        }
    }

    fn call(self, env: &mut Env, args: Vec<Type>) -> Type {
        match self {
            Type::Function(inner_env, arg_names, expression) => {
                let mut evaluated = HashMap::new();
                for (v, n) in args.into_iter().zip(arg_names.iter()) {
                    evaluated.insert(n.clone(), v);
                }
                let mut env = Env {
                    binds: HashMap::new(),
                    evaluated,
                    parent: Some(Rc::new(RefCell::new(inner_env))),
                };
                expression.eval(&mut env)
            }
            Type::Native(n) => n.0.call(env, args),
            _ => unreachable!(),
        }
    }
}

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
    let ast = "
Array.range({start: 1, end: 100})";

    let source = Source::from_str(ast).unwrap();
    println!("{:?}", eval_source(source, None));

    let ast = "fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz \"fizz\" \"\",
  buzz: if is_buzz \"buzz\" \"\",

  fizz.concat(buzz)
},
Array.range({start: 1, end: 100}).map(fizzbuzz)";

    let source = Source::from_str(ast).unwrap();
    println!("{:?}", eval_source(source, None));
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
