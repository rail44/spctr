use crate::types::{BoxedNativeCallable, NativeCallable, Type};
use crate::{list, Env};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct Map(Env, HashMap<String, Type>);

impl Map {
    pub fn new(env: Env, map: HashMap<String, Type>) -> Self {
        Map(env, map)
    }

    pub fn get_prop(&self, name: &str) -> Type {
        let mut child = Env {
            binds: self.1.clone(),
            parent: Some(Rc::new(RefCell::new(self.0.clone()))),
        };
        child.get_value(name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Type {
        Type::Map(
            Map::new(
                Default::default(),
                [("keys".to_string(), BoxedNativeCallable::new(Keys).into())]
                    .iter()
                    .cloned()
                    .collect(),
            )
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keys;

impl NativeCallable for Keys {
    fn call(&self, _env: &mut Env, mut args: Vec<Type>) -> Type {
        if let Type::Map(m) = args.pop().unwrap() {
            return Type::List(list::List::new(
                m.1.keys().map(|k| Type::String(k.to_string())).collect()
            ))
        }
        panic!();
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "List.map")
    }
}
