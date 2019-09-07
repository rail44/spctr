use crate::stack;
use crate::stack::{Env, Function, Unevaluated, Value};
use std::convert::TryInto;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn get_value() -> Value {
        let mut evaluated_map = HashMap::new();
        evaluated_map.insert(
            "parse".to_string(),
            Function::new(Default::default(), vec!["s".to_string()], PARSE).into(),
        );
        let env = Env::new(Default::default(), evaluated_map);

        Value::Map(env)
    }
}

impl std::fmt::Display for JsonModule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JsonModule")
    }
}

pub const PARSE: Unevaluated =
    Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
        let s: String = env.get_value("s")?.try_into()?;
        Ok(stack::eval(&stack::get_stack(&s)?, &mut Env::default())?
            .pop()
            .unwrap())
    });

#[test]
fn test_access_json() {
    let ast = r#"
json_string: "{\"hoge\": [1, 2, null]}",
json: Json.parse(json_string),
json.hoge[2]"#;

    let result = stack::eval(&stack::get_stack(ast).unwrap(), &mut Env::root())
        .unwrap()
        .pop()
        .unwrap();
    println!("{}", result);
    assert!(result == Value::Null);
}
