use crate::types::Type;
use crate::Env;
use std::convert::TryInto;
use std::iter::Iterator;

pub fn concat(env: Env, args: Vec<Type>) -> Result<Type, failure::Error> {
    let base: String = env.get_value("_")?.try_into()?;
    let rights: Result<String, failure::Error> = args
        .into_iter()
        .map(|s| -> Result<String, _> { s.try_into() })
        .collect();
    Ok(Type::String(format!("{}{}", base, rights?)))
}

pub fn split(env: Env, mut args: Vec<Type>) -> Result<Type, failure::Error> {
    let base: String = env.get_value("_")?.try_into()?;
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
