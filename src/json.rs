use crate::types::{BoxedNative, BoxedNativeCallable, Native, NativeCallable, Type};
use crate::list;
use crate::Env;

fn into_value(j: serde_json::Value) -> Type {
    use serde_json::Value;
    match j {
        Value::String(s) => Type::String(s.clone()),
        Value::Number(n) => Type::Number(n.as_f64().unwrap()),
        Value::Array(v) => Type::List(list::List::new(v.into_iter().map(|e| into_value(e)).collect())),
        Value::Object(m) => Type::Map(m.into_iter().map(|(k, v)| (k, into_value(v))).collect()),
        Value::Bool(b) => Type::Boolean(b),
        Value::Null => Type::Null,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn new() -> Type {
        Type::Map(
            [("parse".to_string(), BoxedNativeCallable::new(Parse).into())].iter().cloned().collect()
        )
    }
}

impl std::fmt::Display for JsonModule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JsonModule")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parse;

impl NativeCallable for Parse {
    fn call(&self, _env: &mut Env, mut args: Vec<Type>) -> Type {
        if let Type::String(s) = args.pop().unwrap() {
            return into_value(serde_json::from_str(&s).unwrap());
        }
        panic!();
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Parse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Json.parse")
    }
}
