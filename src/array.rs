use crate::{Env, Type, Native};
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct Array;

impl Native for Array {
    fn comparator(&self) -> &str {
        ""
    }

    fn get_prop(&self, _env: &mut Env, name: &str) -> Type {
        match name {
            "range" => Range.into(),
            _ => unreachable!()
        }
    }

    fn call(&self, _env: &mut Env, _args: Vec<Type>) -> Type {
        panic!();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range;

impl Native for Range {
    fn comparator(&self) -> &str {
        ""
    }

    fn get_prop(&self, env: &mut Env, name: &str) -> Type {
        panic!();
    }

    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        let arg = args.pop().unwrap();
        if let (Type::Number(start), Type::Number(end)) = (arg.get_prop(env, "start"), arg.get_prop(env, "end")) {
            let start = start as i32;
            let end = end as i32;
            return Type::Array((start..end).map(|i| Type::Number(i.into())).collect());
        }
        panic!();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Map(Vec<Type>);

impl Map {
    pub fn new(v: Vec<Type>) -> Self {
        Map(v)
    }
}

impl Native for Map {
    fn comparator(&self) -> &str {
        ""
    }

    fn get_prop(&self, env: &mut Env, name: &str) -> Type {
        panic!();
    }

    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        let arg = args.pop().unwrap();
        Type::Array(self.0.iter().map(|v| arg.clone().call(env, vec![v.clone()])).collect())
    }
}
