use crate::map;
use crate::types::{Native, Type};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct StringModule;

impl StringModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("concat".to_string(), Native::Static(concat).into());
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
