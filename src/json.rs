use crate::types::{BoxedNative, BoxedNativeCallable, Native, NativeCallable, Type};
use crate::Env;

#[derive(Debug, Clone, PartialEq)]
pub struct Json(serde_json::Value);

impl Json {
    pub fn new(v: serde_json::Value) -> Self {
        Json(v)
    }
}

impl Native for Json {
    fn comparator(&self) -> Type {
        Type::String(self.0.to_string())
    }

    fn indexing(&self, _env: &mut Env, i: i32) -> Type {
        use serde_json::Value;
        match self.0.get(i as usize).unwrap() {
            Value::String(s) => Type::String(s.clone()),
            Value::Number(n) => Type::Number(n.as_f64().unwrap()),
            v => BoxedNative::new(Self(v.clone())).into(),
        }
    }

    fn get_prop(&self, _env: &mut Env, name: &str) -> Type {
        use serde_json::Value;
        match self.0.get(name).unwrap() {
            Value::String(s) => Type::String(s.clone()),
            Value::Number(n) => Type::Number(n.as_f64().unwrap()),
            v => BoxedNative::new(Self(v.clone())).into(),
        }
    }

    fn box_clone(&self) -> Box<dyn Native> {
        Box::new(self.clone())
    }
}

impl std::fmt::Display for Json {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Json{}", serde_json::to_string_pretty(&self.0).unwrap())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl Native for JsonModule {
    fn get_prop(&self, _env: &mut Env, name: &str) -> Type {
        match name {
            "parse" => BoxedNativeCallable::new(Parse).into(),
            _ => unreachable!(),
        }
    }

    fn box_clone(&self) -> Box<dyn Native> {
        Box::new(self.clone())
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
            return BoxedNative::new(Json::new(serde_json::from_str(&s).unwrap())).into();
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
