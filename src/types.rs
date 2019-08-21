use crate::eval::Evaluable;
use crate::{map, string, token, Env};

use std::cell::RefCell;
use std::collections::HashMap;

use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Native {
    Static(fn(Vec<Type>) -> Type),
    Method(Box<Type>, fn(Type, Vec<Type>) -> Type),
}

impl Native {
    pub fn call(self, args: Vec<Type>) -> Type {
        match self {
            Native::Static(f) => f(args),
            Native::Method(receiver, f) => f(*receiver, args),
        }
    }
}

impl Into<Type> for Native {
    fn into(self) -> Type {
        Type::Native(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(map::Map),
    Function(Env, Vec<String>, Box<Type>),
    Boolean(bool),
    Native(Native),
    Unevaluated(token::Expression),
    Null,
}

impl Type {
    pub fn get_prop(&self, name: &str) -> Type {
        match self {
            Type::Map(m) => m.get_prop(name),
            Type::String(_s) => match name {
                "concat" => Type::Native(Native::Method(Box::new(self.clone()), string::concat)),
                _ => unreachable!(name),
            },
            _ => unreachable!(name),
        }
    }

    pub fn indexing(&self, n: i32) -> Type {
        match self {
            Type::List(vec) => vec[n as usize].clone(),
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
            Type::Native(n) => n.call(args),
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
            Type::List(vec) => {
                let v: Vec<String> = vec.iter().map(|e| format!("{}", e).to_string()).collect();
                write!(formatter, "[{}]", v.join(", "))
            }
            Type::Function(_, _, _) => write!(formatter, "[function]"),
            Type::Boolean(b) => write!(formatter, "{}", b),
            Type::Native(_n) => write!(formatter, "[Native]"),
            Type::Unevaluated(expression) => write!(formatter, "[Unevaluated {:?}]", expression),
            Type::Null => write!(formatter, "null"),
        }
    }
}
