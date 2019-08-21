use crate::types::{Native, Type};
use std::collections::HashMap;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
pub struct ListModule;

impl ListModule {
    pub fn get_value() -> Type {
        let mut binds = HashMap::new();
        binds.insert("range".to_string(), Native::Static(range).into());
        Type::Map(Default::default(), binds)
    }
}

fn range(mut args: Vec<Type>) -> Type {
    if let (Type::Number(start), Type::Number(end)) = (args.remove(0), args.remove(0)) {
        let start = start as i32;
        let end = end as i32;
        return Type::List((start..end).map(|i| Type::Number(i.into())).collect());
    }
    panic!();
}

pub fn map(receiver: Type, mut args: Vec<Type>) -> Type {
    if let Type::List(l) = receiver {
        let f = args.remove(0);
        return Type::List(l.into_iter().map(|v| f.clone().call(vec![v])).collect());
    }
    panic!();
}

pub fn reduce(receiver: Type, mut args: Vec<Type>) -> Type {
    if let Type::List(l) = receiver {
        let initial = args.remove(0);
        let f = args.remove(0);
        return l
            .into_iter()
            .fold(initial, |acc, v| f.clone().call(vec![acc, v]));
    }
    panic!();
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
    let result = eval_source(source, &mut Default::default());
    println!("{}", result);
    assert_eq!(result, Type::Number(55.0));
}
