use crate::types::{Native, Type};
use crate::Env;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Type {
        Type::Map(
            Default::default(),
            [("keys".to_string(), Native::Static(keys).into())]
                .iter()
                .cloned()
                .collect(),
        )
    }
}

fn keys(mut args: Vec<Type>) -> Type {
    if let Type::Map(env, m) = args.pop().unwrap() {
        return Type::List(m.keys().map(|k| Type::String(k.to_string())).collect());
    }
    panic!();
}
