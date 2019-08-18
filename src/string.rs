use crate::types::Type;
use crate::{map, Env};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct StringModule;

impl StringModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("concat".to_string(), Type::NativeCallable(concat));
        Type::Map(map::Map::new(Default::default(), binds))
    }
}

fn concat(args: Vec<Type>) -> Type {
    Type::String(
        args.into_iter()
            .map(|s| {
                if let Type::String(s) = s {
                    return s;
                }
                panic!();
            })
            .collect(),
    )
}
