use crate::Env;
use crate::types::{Native, NativeCallable, Type, BoxedNativeCallable};
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct List;

impl Native for List {
    fn comparator(&self) -> &str {
        ""
    }

    fn get_prop(&self, _env: &mut Env, name: &str) -> Type {
        match name {
            "range" => BoxedNativeCallable::new(Range).into(),
            _ => unreachable!(),
        }
    }

    fn box_clone(&self) -> Box<dyn Native> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range;

impl NativeCallable for Range {
    fn comparator(&self) -> &str {
        ""
    }

    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        let arg = args.pop().unwrap();
        if let (Type::Number(start), Type::Number(end)) =
            (arg.get_prop(env, "start"), arg.get_prop(env, "end"))
        {
            let start = start as i32;
            let end = end as i32;
            return Type::List((start..end).map(|i| Type::Number(i.into())).collect());
        }
        panic!();
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Map(Vec<Type>);

impl Map {
    pub fn new(v: Vec<Type>) -> Self {
        Map(v)
    }
}

impl NativeCallable for Map {
    fn comparator(&self) -> &str {
        ""
    }

    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        let arg = args.pop().unwrap();
        Type::List(
            self.0
                .iter()
                .map(|v| arg.clone().call(env, vec![v.clone()]))
                .collect(),
        )
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}
