mod list;
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

#[derive(Debug)]
pub struct BoxedNative(Box<dyn Native>);

impl BoxedNative {
    pub fn new<N: Native>(n: N) -> Self {
        BoxedNative(Box::new(n))
    }

    fn get_prop(&self, env: &mut Env, name: &str) -> Type {
        self.0.get_prop(env, name)
    }
}

pub trait Native: 'static + Debug {
    fn get_prop(&self, env: &mut Env, name: &str) -> Type;
    fn comparator(&self) -> &str;
    fn box_clone(&self) -> Box<dyn Native>;
}

impl Clone for BoxedNative {
    fn clone(&self) -> Self {
        BoxedNative(self.0.box_clone())
    }
}

impl From<BoxedNative> for Type {
    fn from(n: BoxedNative) -> Type {
        Type::Native(n)
    }
}

impl PartialEq for BoxedNative {
    fn eq(&self, other: &Self) -> bool {
        self.0.type_id() == other.0.type_id() && self.0.comparator() == other.0.comparator()
    }
}

#[derive(Debug)]
pub struct BoxedNativeCallable(Box<dyn NativeCallable>);

impl BoxedNativeCallable {
    pub fn new<N: NativeCallable>(n: N) -> Self {
        BoxedNativeCallable(Box::new(n))
    }

    fn call(&self, env: &mut Env, args: Vec<Type>) -> Type {
        self.0.call(env, args)
    }
}

pub trait NativeCallable: 'static + Debug {
    fn call(&self, env: &mut Env, args: Vec<Type>) -> Type;
    fn comparator(&self) -> &str;
    fn box_clone(&self) -> Box<dyn NativeCallable>;
}

impl Clone for BoxedNativeCallable {
    fn clone(&self) -> Self {
        BoxedNativeCallable(self.0.box_clone())
    }
}

impl PartialEq for BoxedNativeCallable {
    fn eq(&self, other: &Self) -> bool {
        self.0.type_id() == other.0.type_id() && self.0.comparator() == other.0.comparator()
    }
}

impl From<BoxedNativeCallable> for Type {
    fn from(n: BoxedNativeCallable) -> Type {
        Type::NativeCallable(n)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(HashMap<String, token::Expression>),
    Function(Env, Vec<String>, Box<token::Expression>),
    Boolean(bool),
    Native(BoxedNative),
    NativeCallable(BoxedNativeCallable),
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
            Type::List(v) => match name {
                "map" => BoxedNativeCallable::new(list::Map::new(v.clone())).into(),
                _ => panic!(),
            },
            Type::String(s) => match name {
                "concat" => BoxedNativeCallable::new(string::Concat::new(s.clone())).into(),
                _ => panic!(),
            },
            Type::Native(n) => n.get_prop(env, name),
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
            Type::NativeCallable(n) => n.call(env, args),
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
List.range({start: 1, end: 100})";

    let source = Source::from_str(ast).unwrap();
    println!("{:?}", eval_source(source, None));

    let ast = "fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz \"fizz\" \"\",
  buzz: if is_buzz \"buzz\" \"\",

  fizz.concat(buzz)
},
List.range({start: 1, end: 100}).map(fizzbuzz)";

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
fn test_list() {
    let ast = "[1, \"hoge\"]";
    let source = Source::from_str(ast).unwrap();
    assert!(eval_source(source, None) == Type::List(vec![Type::Number(1.0), Type::String("hoge".to_string())]));
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
