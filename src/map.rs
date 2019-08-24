use crate::types::{FunctionBody, Type};
use crate::Env;
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Type {
        let env = Env::default();
        env.insert(
            "keys".to_string(),
            Type::Function(env.clone(), FunctionBody::Native(keys).into()),
        );
        env.insert(
            "values".to_string(),
            Type::Function(env.clone(), FunctionBody::Native(values).into()),
        );
        Type::Map(env)
    }
}

fn keys(_: Env, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let env: Env = args.pop().unwrap().try_into()?;
    let binds = env.binds.borrow();
    Ok(Type::List(
        binds
            .iter()
            .map(|(k, _)| Type::String(k.to_string()))
            .collect(),
    ))
}

fn values(_: Env, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let env: Env = args.pop().unwrap().try_into()?;
    let binds = env.binds.borrow();
    let members: Result<Vec<_>, _> = binds.iter().map(|(_, v)| v.clone().eval(&env)).collect();
    Ok(Type::List(members?))
}

#[test]
fn test_keys() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
map: {
    "hoge": "HOGE"
},
Map.keys(map)[0]"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
    assert_eq!(result, Type::String("hoge".to_string()));
}

#[test]
fn test_values() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
map: {
    "hoge": "HOGE"
},
Map.values(map)[0]"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
    assert_eq!(result, Type::String("HOGE".to_string()));
}
