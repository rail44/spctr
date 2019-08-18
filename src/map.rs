use crate::types::Type;
use crate::Env;

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
