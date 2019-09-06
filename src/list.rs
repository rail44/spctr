use crate::stack::{Env, Function, Unevaluated, Value};
use std::convert::TryInto;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct ListModule;

impl ListModule {
    pub fn get_value() -> Value {
        let env = Env::default();
        env.insert(
            "range".to_string(),
            Function::new(
                env.clone(),
                vec!["start".to_string(), "end".to_string()],
                RANGE,
            )
            .into(),
        );
        Value::Map(env)
    }
}

pub const RANGE: Unevaluated =
    Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
        let start: f64 = env.get_value("start")?.try_into()?;
        let end: f64 = env.get_value("end")?.try_into()?;
        Ok(Value::List(
            ((start as i32)..(end as i32))
                .map(|i| Value::Number(i.into()))
                .collect(),
        ))
    });

pub const CONCAT: Unevaluated =
    Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
        let mut l: Vec<Value> = env.get_value("_")?.try_into()?;
        let mut other: Vec<Value> = env.get_value("other")?.try_into()?;
        l.append(&mut other);
        Ok(Value::List(l))
    });

#[test]
fn test_count() {
    use crate::stack::{eval, get_stack};

    let ast = r#"
l: List.range(1, 11),
l.count"#;
    let result = eval(&get_stack(ast).unwrap(), &mut Env::root())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(result, Value::Number(10.0));
}
