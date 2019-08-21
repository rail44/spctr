use crate::eval::eval_source;

use crate::token::Source;
use crate::types::{Native, Type};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn get_value() -> Type {
        Type::Map(
            Default::default(),
            [("parse".to_string(), Native::Static(parse).into())]
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

fn parse(mut args: Vec<Type>) -> Type {
    if let Type::String(s) = args.pop().unwrap() {
        return eval_source(Source::from_str(&s).unwrap(), &mut Default::default());
    }
    panic!();
}

#[test]
fn test_access_json() {
    let ast = r#"
json_string: "{\"hoge\": [1, 2, null]}",
json: Json.parse(json_string),
json.hoge[2]"#;

    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default());
    assert_eq!(result, Type::Null);
}
