use crate::types::{BoxedNativeCallable, Native, NativeCallable, Type};
use crate::Env;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct List(Vec<Type>);

impl List {
    pub fn new(v: Vec<Type>) -> Self {
        List(v)
    }

    pub fn indexing(&self, n: f64) -> Type {
        self.0[n as usize].clone()
    }
}

impl std::fmt::Display for List {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let v: Vec<String> = self
            .0
            .iter()
            .map(|e| format!("{}", e).to_string())
            .collect();
        write!(f, "[{}]", v.join(", "))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListModule;

impl Native for ListModule {
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

impl std::fmt::Display for ListModule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ListModule")
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
            return Type::List(List::new(
                (start..=end).map(|i| Type::Number(i.into())).collect(),
            ));
        }
        panic!();
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "List.range")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Map(List);

impl Map {
    pub fn new(l: List) -> Self {
        Map(l)
    }
}

impl NativeCallable for Map {
    fn comparator(&self) -> &str {
        ""
    }

    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        let arg = args.pop().unwrap();
        Type::List(List::new(
            (self.0)
                .0
                .iter()
                .map(|v| arg.clone().call(env, vec![v.clone()]))
                .collect(),
        ))
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}.map", self.0)
    }
}
