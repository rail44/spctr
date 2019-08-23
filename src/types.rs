use crate::eval::Evaluable;
use crate::{list, string, token, Env};

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::rc::Rc;

use failure::format_err;

#[derive(Debug, Clone, PartialEq)]
pub enum Native {
    Static(fn(Vec<Type>) -> Result<Type, failure::Error>),
    Method(
        Box<Type>,
        fn(Type, Vec<Type>) -> Result<Type, failure::Error>,
    ),
}

impl Native {
    pub fn call(self, args: Vec<Type>) -> Result<Type, failure::Error> {
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

pub type Map = (Env, HashMap<String, Type>);

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(Env, HashMap<String, Type>),
    Function(Env, Vec<String>, Box<Type>),
    Boolean(bool),
    Native(Native),
    Unevaluated(token::Expression),
    Null,
}

impl Type {
    pub fn get_prop(&self, name: &str) -> Result<Type, failure::Error> {
        match self {
            Type::Map(env, m) => {
                let mut child = Env {
                    binds: m.clone(),
                    parent: Some(Rc::new(RefCell::new(env.clone()))),
                };
                child.get_value(name)
            }
            Type::String(_s) => match name {
                "concat" => Ok(Type::Native(Native::Method(
                    Box::new(self.clone()),
                    string::concat,
                ))),
                "split" => Ok(Type::Native(Native::Method(
                    Box::new(self.clone()),
                    string::split,
                ))),
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            Type::List(_v) => match name {
                "map" => Ok(Type::Native(Native::Method(
                    Box::new(self.clone()),
                    list::map,
                ))),
                "reduce" => Ok(Type::Native(Native::Method(
                    Box::new(self.clone()),
                    list::reduce,
                ))),
                "find" => Ok(Type::Native(Native::Method(
                    Box::new(self.clone()),
                    list::find,
                ))),
                "filter" => Ok(Type::Native(Native::Method(
                    Box::new(self.clone()),
                    list::filter,
                ))),
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            _ => Err(format_err!("{} has no prop `{}`", self, name)),
        }
    }

    pub fn indexing(&self, n: i32) -> Result<Type, failure::Error> {
        match self {
            Type::List(vec) => Ok(vec[n as usize].clone()),
            _ => Err(format_err!("{} has no index {}", self, n)),
        }
    }

    pub fn call(self, args: Vec<Type>) -> Result<Type, failure::Error> {
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
            _ => Err(format_err!("{} is not callable", self)),
        }
    }

    pub fn eval(self, env: &mut Env) -> Result<Type, failure::Error> {
        match self {
            Type::Unevaluated(expression) => expression.eval(env),
            _ => Ok(self),
        }
    }
}

impl TryInto<String> for Type {
    type Error = failure::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        if let Type::String(s) = self {
            return Ok(s);
        }
        Err(format_err!("{} is not String", self))
    }
}

impl TryInto<Vec<Type>> for Type {
    type Error = failure::Error;

    fn try_into(self) -> Result<Vec<Type>, Self::Error> {
        if let Type::List(v) = self {
            return Ok(v);
        }
        Err(format_err!("{} is not List", self))
    }
}

impl TryInto<f64> for Type {
    type Error = failure::Error;

    fn try_into(self) -> Result<f64, Self::Error> {
        if let Type::Number(f) = self {
            return Ok(f);
        }
        Err(format_err!("{} is not Number", self))
    }
}

impl TryInto<Map> for Type {
    type Error = failure::Error;

    fn try_into(self) -> Result<Map, Self::Error> {
        if let Type::Map(env, map) = self {
            return Ok((env, map));
        }
        Err(format_err!("{} is not Map", self))
    }
}

impl TryInto<bool> for Type {
    type Error = failure::Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        if let Type::Boolean(b) = self {
            return Ok(b);
        }
        Err(format_err!("{} is not Boolean", self))
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Number(f) => write!(formatter, "{}", f),
            Type::String(s) => write!(formatter, "\"{}\"", s),
            Type::Map(_env, m) => write!(formatter, "{:?}", m),
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
