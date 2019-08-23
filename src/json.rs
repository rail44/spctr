use crate::eval::eval_source;
use crate::token::Source;
use crate::types::{Native, Type};
use crate::Env;
use std::convert::TryInto;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn get_value() -> Type {
        let mut env = Env::default();
        env.binds
            .insert("parse".to_string(), Native::Static(parse).into());

        Type::Map(env)
    }
}

impl std::fmt::Display for JsonModule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JsonModule")
    }
}

fn parse(mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let s: String = args.pop().unwrap().try_into()?;
    eval_source(Source::from_str(&s).unwrap(), &mut Default::default())
}

#[test]
fn test_access_json() {
    let ast = r#"
json_string: "{\"hoge\": [1, 2, null]}",
json: Json.parse(json_string),
json.hoge[2]"#;

    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
    println!("{}", result);
    assert!(result == Type::Null);
}
