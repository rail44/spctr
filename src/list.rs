use crate::types::{BoxedNativeCallable, NativeCallable, Type};
use crate::Env;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct List(Vec<Type>);

impl List {
    pub fn new(v: Vec<Type>) -> Self {
        List(v)
    }

    pub fn indexing(&self, n: i32) -> Type {
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

impl ListModule {
    pub fn new() -> Type {
        Type::Map(
            Default::default(),
            [("range".to_string(), BoxedNativeCallable::new(Range).into())]
                .iter()
                .cloned()
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range;

impl NativeCallable for Range {
    fn call(&self, _env: &mut Env, mut args: Vec<Type>) -> Type {
        if let (Type::Number(start), Type::Number(end)) =
            (args.remove(0), args.remove(0))
        {
            let start = start as i32;
            let end = end as i32;
            return Type::List(List::new(
                (start..end).map(|i| Type::Number(i.into())).collect(),
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
    fn comparator(&self) -> Type {
        Type::List(self.0.clone())
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
