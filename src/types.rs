use crate::eval::Evaluable;
use crate::{list, string, token, Env};

use std::collections::HashMap;
use std::convert::TryInto;

use failure::format_err;

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    Expression(Vec<String>, Box<Type>),
    Native(fn(Env, Vec<Type>) -> Result<Type, failure::Error>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(Env),
    Function(Env, FunctionBody),
    Boolean(bool),
    Unevaluated(token::Expression),
    Null,
}

impl Type {
    pub fn get_prop(&mut self, name: &str) -> Result<Type, failure::Error> {
        let env: Env = Default::default();
        env.insert("_".to_string(), self.clone());
        match self {
            Type::Map(env) => env.clone().get_value(name),
            Type::String(_s) => match name {
                "concat" => Ok(Type::Function(env, FunctionBody::Native(string::concat))),
                "split" => Ok(Type::Function(env, FunctionBody::Native(string::split))),
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            Type::List(v) => match name {
                "map" => Ok(Type::Function(env, FunctionBody::Native(list::map))),
                "reduce" => Ok(Type::Function(env, FunctionBody::Native(list::reduce))),
                "find" => Ok(Type::Function(env, FunctionBody::Native(list::find))),
                "filter" => Ok(Type::Function(env, FunctionBody::Native(list::filter))),
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
            Type::Function(inner_env, FunctionBody::Expression(arg_names, body)) => {
                let mut binds = HashMap::new();
                for (v, n) in args.into_iter().zip(arg_names.iter()) {
                    binds.insert(n.clone(), v);
                }
                let mut env = inner_env.spawn_child(binds);
                body.eval(&mut env)
            }
            Type::Function(env, FunctionBody::Native(f)) => f(env, args),
            _ => Err(format_err!("{} is not callable", self)),
        }
    }

    pub fn eval(self, env: &Env) -> Result<Type, failure::Error> {
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
            Type::Function(_, _) => write!(formatter, "[function]"),
            Type::Boolean(b) => write!(formatter, "{}", b),
            Type::Unevaluated(expression) => write!(formatter, "[Unevaluated {:?}]", expression),
            Type::Null => write!(formatter, "null"),
        }
    }
}
