use crate::stack;
use crate::stack::{Env, Function, Unevaluated, Value};
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn get_value() -> Value {
        let env = Env::default();
        env.insert(
            "parse".to_string(),
            Function::new(env.clone(), vec!["s".to_string()], PARSE).into(),
        );

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
