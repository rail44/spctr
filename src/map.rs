use std::collections::HashMap;
use crate::types::{Native, Type};

#[derive(Debug, Clone, PartialEq)]
pub struct MapModule;

impl MapModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("keys".to_string(), Native::Static(keys).into());
        binds.insert("values".to_string(), Native::Static(values).into());
        Type::Map(
            Default::default(),
            binds
        )
    }
}

fn keys(mut args: Vec<Type>) -> Type {
    if let Type::Map(_env, m) = args.pop().unwrap() {
        return Type::List(m.into_iter().map(|(k, _)| Type::String(k)).collect());
    }
    panic!();
}

fn values(mut args: Vec<Type>) -> Type {
    if let Type::Map(mut env, m) = args.pop().unwrap() {
        return Type::List(m.into_iter().map(|(_, v)| v.eval(&mut env)).collect());
    }
    panic!();
}

#[test]
fn test_keys() {
    use crate::token::Source;
    use crate::eval::eval_source;
    use std::str::FromStr;

    let ast = r#"
map: {
    "hoge": "HOGE"
},
Map.keys(map)[0]"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default());
    println!("{}", result);
    assert!(result == Type::String("hoge".to_string()));
}

#[test]
fn test_values() {
    use crate::token::Source;
    use crate::eval::eval_source;
    use std::str::FromStr;

    let ast = r#"
map: {
    "hoge": "HOGE"
},
Map.values(map)[0]"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default());
    println!("{}", result);
    assert!(result == Type::String("HOGE".to_string()));
}
