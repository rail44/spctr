use crate::eval::Evaluable;
use crate::{list, string, token, Env};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::rc::Rc;

#[derive(Debug)]
pub struct BoxedNativeCallable(Box<dyn NativeCallable>);

impl BoxedNativeCallable {
    pub fn new<N: NativeCallable>(n: N) -> Self {
        BoxedNativeCallable(Box::new(n))
    }

    pub fn call(&self, env: &mut Env, args: Vec<Type>) -> Type {
        self.0.call(env, args)
    }
}

pub trait NativeCallable: 'static + Debug + Display {
    fn call(&self, _env: &mut Env, _args: Vec<Type>) -> Type {
        unimplemented!()
    }

    fn comparator(&self) -> Type {
        Type::Number(0.0)
    }

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
    List(list::List),
    Map(Env, HashMap<String, Type>),
    Function(Env, Vec<String>, Box<Type>),
    Boolean(bool),
    NativeCallable(BoxedNativeCallable),
    Unevaluated(token::Expression),
    Null,
}

impl Type {
    pub fn get_prop(&self, _env: &mut Env, name: &str) -> Type {
        match self {
            Type::Map(env, map) => {
                let mut child = Env {
                    binds: map.clone(),
                    parent: Some(Rc::new(RefCell::new(env.clone()))),
                };
                child.get_value(name)
            }
            Type::List(l) => match name {
                "map" => BoxedNativeCallable::new(list::Map::new(l.clone())).into(),
                _ => panic!(),
            },
            Type::String(s) => match name {
                "concat" => BoxedNativeCallable::new(string::Concat::new(s.clone())).into(),
                _ => panic!(),
            },
            _ => unreachable!(),
        }
    }

    pub fn indexing(&self, n: i32) -> Type {
        match self {
            Type::List(l) => l.indexing(n),
            _ => unreachable!(),
        }
    }

    pub fn call(self, env: &mut Env, args: Vec<Type>) -> Type {
        match self {
            Type::Function(inner_env, arg_names, expression) => {
                let mut binds = HashMap::new();
                for (v, n) in args.into_iter().zip(arg_names.iter()) {
                    binds.insert(n.clone(), v);
                }
                let mut env = Env {
                    binds,
                    parent: Some(Rc::new(RefCell::new(inner_env))),
                };
                expression.eval(&mut env)
            }
            Type::NativeCallable(n) => n.call(env, args),
            _ => unreachable!(),
        }
    }

    pub fn eval(self, env: &mut Env) -> Type {
        match self {
            Type::Unevaluated(expression) => expression.eval(env),
            _ => self,
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Number(f) => write!(formatter, "{}", f),
            Type::String(s) => write!(formatter, "\"{}\"", s),
            Type::Map(_env, m) => write!(formatter, "{:?}", m),
            Type::List(l) => write!(formatter, "{}", l),
            Type::Function(_, _, _) => write!(formatter, "[function]"),
            Type::Boolean(b) => write!(formatter, "{}", b),
            Type::NativeCallable(n) => write!(formatter, "[NativeCallable {}]", n.0),
            Type::Unevaluated(expression) => write!(formatter, "[Unevaluated {:?}]", expression),
            Type::Null => write!(formatter, "null"),
        }
    }
}
