use crate::{list, string, Env, Unevaluated};

use std::collections::HashMap;
use std::convert::TryInto;

use failure::format_err;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(Env),
    Function(Env, Vec<String>, Unevaluated),
    Boolean(bool),
    Null,
}

impl Type {
    pub fn get_prop(&mut self, name: &str) -> Result<Type, failure::Error> {
        let env: Env = Default::default();
        env.insert("_".to_string(), self.clone());
        match self {
            Type::Map(env) => env
                .clone()
                .get_value(name)
                .map_err(|_| format_err!("{} has no prop `{}`", self, name)),
            Type::String(_s) => match name {
                "concat" => Ok(Type::Function(
                    env,
                    vec!["other".to_string()],
                    string::CONCAT,
                )),
                "split" => Ok(Type::Function(env, vec!["pat".to_string()], string::SPLIT)),
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            Type::List(v) => match name {
                "map" => Ok(Type::Function(env, vec!["f".to_string()], list::MAP)),
                "reduce" => Ok(Type::Function(
                    env,
                    vec!["initial".to_string(), "f".to_string()],
                    list::REDUCE,
                )),
                "find" => Ok(Type::Function(env, vec!["f".to_string()], list::FIND)),
                "filter" => Ok(Type::Function(env, vec!["f".to_string()], list::FILTER)),
                "concat" => Ok(Type::Function(env, vec!["other".to_string()], list::CONCAT)),
                "count" => Ok(Type::Number(v.len() as f64)),
                "to_iter" => {
                    fn next(env: Env) -> Result<Type, failure::Error> {
                        let i: f64 = env.get_value("i")?.try_into()?;
                        let list: Vec<Type> = env.get_value("list")?.try_into()?;

                        Ok(list.get(i as usize).map_or(Type::Null, |v| {
                            let new_env: Env = Default::default();
                            new_env.insert("list".to_string(), Type::List(list.clone()));
                            new_env.insert("i".to_string(), Type::Number(i + 1.0));
                            new_env
                                .bind_map
                                .borrow_mut()
                                .insert("next".to_string(), Unevaluated::Native(next));

                            Type::List(vec![Type::Map(new_env), v.clone()])
                        }))
                    }

                    let new_env: Env = Default::default();
                    new_env.insert("list".to_string(), self.clone());
                    new_env.insert("i".to_string(), Type::Number(0.0));
                    new_env
                        .bind_map
                        .borrow_mut()
                        .insert("next".to_string(), Unevaluated::Native(next));
                    Ok(Type::Map(new_env))
                }
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
                let mut evaluated = HashMap::new();
                for (v, n) in args.into_iter().zip(arg_names.iter()) {
                    evaluated.insert(n.clone(), v);
                }
                let mut env = Env::new(Default::default(), evaluated);
                env.parents.push(inner_env);
                body.eval(&mut env)
            }
            _ => Err(format_err!("{} is not callable", self)),
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
            Type::Map(env) => {
                let bind_map = env.clone().bind_map.borrow().clone();
                let pairs: Vec<String> = bind_map
                    .keys()
                    .map(|k| {
                        let v = env.get_value(k).unwrap();
                        format!("\"{}\": {}", k, v)
                    })
                    .collect();
                write!(formatter, "{{{}}}", pairs.join(", "))
            }
            Type::List(vec) => {
                let v: Vec<String> = vec.iter().map(|e| format!("{}", e).to_string()).collect();
                write!(formatter, "[{}]", v.join(", "))
            }
            Type::Function(_, _, _) => write!(formatter, "[function]"),
            Type::Boolean(b) => write!(formatter, "{}", b),
            Type::Null => write!(formatter, "null"),
        }
    }
}
