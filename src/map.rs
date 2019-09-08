use crate::stack::{Env, Function, Unevaluated, Value};
use std::convert::TryInto;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Value {
        let mut evaluated_map = HashMap::new();
        evaluated_map.insert(
            "keys".to_string(),
            Function::new(Default::default(), vec!["map".to_string()], KEYS).into(),
        );
        evaluated_map.insert(
            "values".to_string(),
            Function::new(Default::default(), vec!["map".to_string()], VALUES).into(),
        );
        let env = Env::new(Default::default(), evaluated_map);

        Value::Map(env)
    }
}

const KEYS: Unevaluated = Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
    let map: Env = env.get_value("map")?.try_into()?;
    let binds = map.bind_map.borrow();
    Ok(Value::List(
        binds
            .iter()
            .map(|(k, _)| Value::String(k.to_string()))
            .collect(),
    ))
});

const VALUES: Unevaluated = Unevaluated::Native(|mut env: Env| -> Result<Value, failure::Error> {
    let map: Env = env.get_value("map")?.try_into()?;
    let binds = map.bind_map.borrow();

    let mut values = vec![];
    for (_, v) in binds.iter() {
        values.push(v.eval(env.clone())?);
    }
    Ok(Value::List(values))
});

#[test]
fn test_keys() {
    use crate::stack::{eval, get_stack, Env};

    let ast = r#"
map: {
    "hoge": "HOGE"
},
Map.keys(map)[0]"#;
    let result = eval(&get_stack(ast).unwrap(), &mut Env::root())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(result, Value::String("hoge".to_string()));
}

#[test]
fn test_values() {
    use crate::stack::{eval, get_stack, Env};

    let ast = r#"
map: {
    "hoge": "HOGE"
},
Map.values(map)[0]"#;
    let result = eval(&get_stack(ast).unwrap(), &mut Env::root())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(result, Value::String("HOGE".to_string()));
}
