use crate::types::{Native, Type};

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
    if let Type::Map(_env, m) = args.pop().unwrap() {
        return Type::List(m.keys().map(|k| Type::String(k.to_string())).collect());
    }
    panic!();
}
