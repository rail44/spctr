use crate::stack::{Env, Unevaluated, Value};
use std::convert::TryInto;
use std::iter::Iterator;

pub const CONCAT: Unevaluated =
    Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
        let base: String = env.get_value("_")?.try_into()?;
        let other: String = env.get_value("other")?.try_into()?;
        Ok(Value::String(format!("{}{}", base, other)))
    });

pub const SPLIT: Unevaluated =
    Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
        let base: String = env.get_value("_")?.try_into()?;
        let pat: String = env.get_value("pat")?.try_into()?;
        Ok(Value::List(
            base.split(&pat)
                .map(|s| Value::String(s.to_string()))
                .collect(),
        ))
    });

#[test]
fn test_split() {
    use crate::stack::{eval, get_stack};

    let ast = r#"
hoge: "HOGE,hoge",
hoge.split(",")"#;
    let result = eval(&get_stack(ast).unwrap(), &mut Default::default())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        result,
        Value::List(vec![
            Value::String("HOGE".to_string()),
            Value::String("hoge".to_string())
        ])
    );
}
