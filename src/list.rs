use crate::types::Type;
use crate::{Env, Unevaluated};
use std::convert::TryInto;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct ListModule;

impl ListModule {
    pub fn get_value() -> Type {
        let env = Env::default();
        env.insert(
            "range".to_string(),
            Type::Function(
                env.clone(),
                vec!["start".to_string(), "end".to_string()],
                RANGE,
            ),
        );
        Type::Map(env)
    }
}

pub const RANGE: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let start: f64 = env.get_value("start")?.try_into()?;
    let end: f64 = env.get_value("end")?.try_into()?;
    Ok(Type::List(
        ((start as i32)..(end as i32))
            .map(|i| Type::Number(i.into()))
            .collect(),
    ))
});

pub const MAP: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let l: Vec<Type> = env.get_value("_")?.try_into()?;
    let f = env.get_value("f")?;
    let members: Result<Vec<_>, _> = l.into_iter().map(|v| f.clone().call(vec![v])).collect();
    Ok(Type::List(members?))
});

pub const REDUCE: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let l: Vec<Type> = env.get_value("_")?.try_into()?;
    let initial = env.get_value("initial")?;
    let f = env.get_value("f")?;
    Ok(l.into_iter()
        .try_fold(initial, |acc, v| f.clone().call(vec![acc, v]))?)
});

pub const FIND: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let l: Vec<Type> = env.get_value("_")?.try_into()?;
    let f = env.get_value("f")?;
    for v in l {
        let b: bool = f.clone().call(vec![v.clone()])?.try_into()?;
        if b {
            return Ok(v);
        }
    }
    Ok(Type::Null)
});

pub const FILTER: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let l: Vec<Type> = env.get_value("_")?.try_into()?;
    let f = env.get_value("f")?;
    let mut result = vec![];
    for v in l {
        let b: bool = f.clone().call(vec![v.clone()])?.try_into()?;
        if b {
            result.push(v);
        }
    }
    Ok(Type::List(result))
});

#[test]
fn test_reduce() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
l: List.range(1, 11),
l.reduce(0, (sum, i) => sum + i)"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    assert_eq!(result, Type::Number(55.0));
}

#[test]
fn test_find() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
l: List.range(3, 11),
l.find((i) => i % 7 = 1)"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    assert_eq!(result, Type::Number(8.0));
}

#[test]
fn test_filter() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
l: List.range(1, 11),
l.filter((i) => i % 3 = 0)"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    assert_eq!(
        result,
        Type::List(vec![
            Type::Number(3.0),
            Type::Number(6.0),
            Type::Number(9.0)
        ])
    );
}

#[test]
fn test_count() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
l: List.range(1, 11),
l.filter((i) => i % 3 = 0).count"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    assert_eq!(result, Type::Number(3.0));
}
