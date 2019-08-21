use crate::map;
use crate::types::{Native, Type};
use std::collections::HashMap;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct ListModule;

impl ListModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("range".to_string(), Native::Static(range).into());
        Type::Map(map::Map::new(Default::default(), binds))
    }
}

fn range(mut args: Vec<Type>) -> Type {
    if let (Type::Number(start), Type::Number(end)) = (args.remove(0), args.remove(0)) {
        let start = start as i32;
        let end = end as i32;
        return Type::List((start..end).map(|i| Type::Number(i.into())).collect());
    }
    panic!();
}

pub fn map(mut args: Vec<Type>) -> Type {
    if let Type::List(l) = args.remove(0) {
        let f = args.remove(0);
        return Type::List(l.into_iter().map(|v| f.clone().call(vec![v])).collect());
    }
    panic!();
}
