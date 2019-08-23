use crate::types::{Native, Type};
use crate::Env;
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Type {
        let mut env = Env::default();
        env.binds
            .insert("keys".to_string(), Native::Static(keys).into());
        env.binds
            .insert("values".to_string(), Native::Static(values).into());
        Type::Map(env)
    }
}

fn keys(mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let env: Env = args.pop().unwrap().try_into()?;
    Ok(Type::List(
        env.binds
            .into_iter()
            .map(|(k, _)| Type::String(k))
            .collect(),
    ))
}

fn values(mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let mut env: Env = args.pop().unwrap().try_into()?;
    let members: Result<Vec<_>, _> = env
        .binds
        .clone()
        .into_iter()
        .map(|(_, v)| v.eval(&mut env))
        .collect();
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
