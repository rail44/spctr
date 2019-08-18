use crate::eval::Evaluable;
use crate::{list, map, token, Env};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(list::List),
    Map(map::Map),
    Function(Env, Vec<String>, Box<Type>),
    Boolean(bool),
    NativeCallable(fn(Vec<Type>) -> Type),
    Unevaluated(token::Expression),
    Null,
}

impl Type {
    pub fn get_prop(&self, name: &str) -> Type {
        match self {
            Type::Map(m) => m.get_prop(name),
            _ => unreachable!(name),
        }
    }

    pub fn indexing(&self, n: i32) -> Type {
        match self {
            Type::List(l) => l.indexing(n),
            _ => unreachable!(),
        }
    }

    pub fn call(self, args: Vec<Type>) -> Type {
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
            Type::NativeCallable(n) => n(args),
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
            Type::Map(m) => write!(formatter, "{:?}", m),
            Type::List(l) => write!(formatter, "{}", l),
            Type::Function(_, _, _) => write!(formatter, "[function]"),
            Type::Boolean(b) => write!(formatter, "{}", b),
            Type::NativeCallable(n) => write!(formatter, "[NativeCallable]"),
            Type::Unevaluated(expression) => write!(formatter, "[Unevaluated {:?}]", expression),
            Type::Null => write!(formatter, "null"),
        }
    }
}
