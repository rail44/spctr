use crate::types::{BoxedNativeCallable, NativeCallable, Type};
use crate::Env;
use crate::token::{Source};
use crate::eval::{eval_source};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn new() -> Type {
        Type::Map(
            [("parse".to_string(), BoxedNativeCallable::new(Parse).into())]
                .iter()
                .cloned()
                .collect(),
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
    fn call(&self, env: &mut Env, mut args: Vec<Type>) -> Type {
        if let Type::String(s) = args.pop().unwrap() {
            return eval_source(Source::from_str(&s).unwrap(), Some(env));
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


#[test]
fn test_access_json() {
    let ast = r#"
json_string: "{\"hoge\": [1, 2, null]}",
json: Json.parse(json_string),
json.hoge[2]"#;

    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, None);
    println!("{}", result);
    assert!(result == Type::Null);
}
