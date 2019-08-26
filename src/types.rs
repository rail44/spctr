use crate::eval::Evaluable;
use crate::{list, string, token, Env};

use std::collections::HashMap;
use std::convert::TryInto;

use failure::format_err;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(Env),
    Function(Env, Vec<String>, Box<Type>),
    Boolean(bool),
    Unevaluated(token::Expression),
    Native(fn(Env) -> Result<Type, failure::Error>),
    Null,
}

impl Type {
    pub fn get_prop(&mut self, name: &str) -> Result<Type, failure::Error> {
        let env: Env = Default::default();
        env.insert("_".to_string(), self.clone());
        match self {
            Type::Map(env) => env.clone().get_value(name),
            Type::String(_s) => match name {
                "concat" => Ok(Type::Function(
                    env,
                    vec!["other".to_string()],
                    Box::new(string::CONCAT),
                )),
                "split" => Ok(Type::Function(
                    env,
                    vec!["pat".to_string()],
                    Box::new(string::SPLIT),
                )),
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            Type::List(v) => match name {
                "map" => Ok(Type::Function(
                    env,
                    vec!["f".to_string()],
                    Box::new(list::MAP),
                )),
                "reduce" => Ok(Type::Function(
                    env,
                    vec!["initial".to_string(), "f".to_string()],
                    Box::new(list::REDUCE),
                )),
                "find" => Ok(Type::Function(
                    env,
                    vec!["f".to_string()],
                    Box::new(list::FIND),
                )),
                "filter" => Ok(Type::Function(
                    env,
                    vec!["f".to_string()],
                    Box::new(list::FILTER),
                )),
                "count" => Ok(Type::Number(v.len() as f64)),
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
            Type::Function(inner_env, arg_names, body) => {
                let mut binds = HashMap::new();
                for (v, n) in args.into_iter().zip(arg_names.iter()) {
                    binds.insert(n.clone(), v);
                }
                let mut env = inner_env.spawn_child(binds);
                body.eval(&mut env)
            }
            _ => Err(format_err!("{} is not callable", self)),
        }
    }

    pub fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        match self {
            Type::Unevaluated(expression) => expression.eval(env),
            Type::Native(f) => f(env.clone()),
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

impl TryInto<Env> for Type {
    type Error = failure::Error;

    fn try_into(self) -> Result<Env, Self::Error> {
        if let Type::Map(env) = self {
            return Ok(env);
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
            Type::Map(env) => write!(formatter, "{:?}", env),
            Type::List(vec) => {
                let v: Vec<String> = vec.iter().map(|e| format!("{}", e).to_string()).collect();
                write!(formatter, "[{}]", v.join(", "))
            }
            Type::Function(_, _, _) => write!(formatter, "[function]"),
            Type::Boolean(b) => write!(formatter, "{}", b),
            Type::Unevaluated(expression) => write!(formatter, "[Unevaluated {:?}]", expression),
            Type::Native(f) => write!(formatter, "[Native {:?}]", f),
            Type::Null => write!(formatter, "null"),
        }
    }
}
