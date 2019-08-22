use crate::types::Type;
use std::convert::TryInto;
use std::iter::Iterator;

pub fn concat(receiver: Type, args: Vec<Type>) -> Result<Type, failure::Error> {
    let base: String = receiver.try_into()?;
    let rights: String = args
        .into_iter()
        .map(|s| {
            if let Type::String(s) = s {
                return s;
            }
            panic!();
        })
        .collect();
    Ok(Type::String(format!("{}{}", base, rights)))
}

pub fn split(receiver: Type, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let base: String = receiver.try_into()?;
    let pat: String = args.remove(0).try_into()?;
    Ok(Type::List(
        base.split(&pat)
            .map(|s| Type::String(s.to_string()))
            .collect(),
    ))
}

#[test]
fn test_split() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
hoge: "HOGE,hoge",
hoge.split(",")"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Default::default()).unwrap();
    assert_eq!(
        result,
        Type::List(vec![
            Type::String("HOGE".to_string()),
            Type::String("hoge".to_string())
        ])
    );
}
