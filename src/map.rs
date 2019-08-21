use crate::types::{Map, Native, Type};
use std::collections::HashMap;
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("keys".to_string(), Native::Static(keys).into());
        binds.insert("values".to_string(), Native::Static(values).into());
        Type::Map(Default::default(), binds)
    }
}

fn keys(mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let (_env, m): Map = args.pop().unwrap().try_into()?;
    Ok(Type::List(
        m.into_iter().map(|(k, _)| Type::String(k)).collect(),
    ))
}

fn values(mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let (mut env, m): Map = args.pop().unwrap().try_into()?;
    Ok(Type::List(
        m.into_iter().map(|(_, v)| v.eval(&mut env)).collect(),
    ))
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
    let result = eval_source(source, &mut Default::default());
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
    let result = eval_source(source, &mut Default::default());
    assert_eq!(result, Type::String("HOGE".to_string()));
}
