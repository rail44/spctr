use crate::types::{FunctionBody, Type};
use crate::Env;
use std::convert::TryInto;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct ListModule;

impl ListModule {
    pub fn get_value() -> Type {
        let mut env = Env::default();
        env.binds.insert(
            "range".to_string(),
            Type::Function(
                env.clone(),
                FunctionBody::Native(Box::new(Type::Null), range).into(),
            ),
        );
        Type::Map(env)
    }
}

fn range(_: Type, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let start: f64 = args.remove(0).try_into()?;
    let end: f64 = args.remove(0).try_into()?;
    Ok(Type::List(
        ((start as i32)..(end as i32))
            .map(|i| Type::Number(i.into()))
            .collect(),
    ))
}

pub fn map(receiver: Type, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let l: Vec<Type> = receiver.try_into()?;
    let f = args.remove(0);
    let members: Result<Vec<_>, _> = l.into_iter().map(|v| f.clone().call(vec![v])).collect();
    Ok(Type::List(members?))
}

pub fn reduce(receiver: Type, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let l: Vec<Type> = receiver.try_into()?;
    let initial = args.remove(0);
    let f = args.remove(0);
    Ok(l.into_iter()
        .try_fold(initial, |acc, v| f.clone().call(vec![acc, v]))?)
}

pub fn find(receiver: Type, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let l: Vec<Type> = receiver.try_into()?;
    let f = args.remove(0);
    for v in l {
        let b: bool = f.clone().call(vec![v.clone()])?.try_into()?;
        if b {
            return Ok(v);
        }
    }
    Ok(Type::Null)
}

pub fn filter(receiver: Type, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let l: Vec<Type> = receiver.try_into()?;
    let f = args.remove(0);
    let mut result = vec![];
    for v in l {
        let b: bool = f.clone().call(vec![v.clone()])?.try_into()?;
        if b {
            result.push(v);
        }
    }
    Ok(Type::List(result))
}

#[test]
fn test_reduce() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
l: List.range(1, 11),
l.reduce(0, (sum, i) => sum + i)"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
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
    let result = eval_source(source, &mut Default::default()).unwrap();
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
    let result = eval_source(source, &mut Default::default()).unwrap();
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
    let result = eval_source(source, &mut Default::default()).unwrap();
    assert_eq!(result, Type::Number(3.0));
}
