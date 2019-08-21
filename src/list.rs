use crate::map;
use crate::types::{Native, Type};
use std::collections::HashMap;
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
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("range".to_string(), Native::Static(range).into());
        binds.insert("map".to_string(), Native::Static(map).into());
        Type::Map(map::Map::new(Default::default(), binds))
    }
}

fn range(mut args: Vec<Type>) -> Type {
    if let (Type::Number(start), Type::Number(end)) = (args.remove(0), args.remove(0)) {
        let start = start as i32;
        let end = end as i32;
        return Type::List(List::new(
            (start..end).map(|i| Type::Number(i.into())).collect(),
        ));
    }
    panic!();
}

fn map(mut args: Vec<Type>) -> Type {
    if let Type::List(l) = args.remove(0) {
        let f = args.remove(0);
        return Type::List(List::new(
            l.0.iter()
                .map(|v| f.clone().call(vec![v.clone()]))
                .collect(),
        ));
    }
    panic!();
}
