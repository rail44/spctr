use crate::types::{BoxedNativeCallable, NativeCallable, Type};
use crate::{map, Env};
use std::iter::Iterator;
use std::collections::HashMap;

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
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("range".to_string(), BoxedNativeCallable::new(Range).into());
        binds.insert("map".to_string(), BoxedNativeCallable::new(Map).into());
        Type::Map(map::Map::new(
            Default::default(),
            binds
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range;

impl NativeCallable for Range {
    fn call(&self, _env: &mut Env, mut args: Vec<Type>) -> Type {
        if let (Type::Number(start), Type::Number(end)) = (args.remove(0), args.remove(0)) {
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
pub struct Map;

impl NativeCallable for Map {
    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        if let Type::List(l) = args.remove(0) {
            let f = args.remove(0);
            return Type::List(List::new(
                l.0
                    .iter()
                    .map(|v| f.clone().call(env, vec![v.clone()]))
                    .collect(),
            ))
        }
        panic!();
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "List.map")
    }
}
