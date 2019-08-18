use crate::types::{BoxedNativeCallable, NativeCallable, Type};
use crate::{map, Env};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct StringModule;

impl StringModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("concat".to_string(), BoxedNativeCallable::new(Concat).into());
        Type::Map(map::Map::new(
            Default::default(),
            binds
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Concat;

impl NativeCallable for Concat {
    fn call(&self, _env: &mut Env, args: Vec<Type>) -> Type {
        Type::String(args.into_iter().map(|s| {
            if let Type::String(s) = s {
                return s
            }
            panic!();
        }).collect())
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Concat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "String.concat")
    }
}
