use crate::types::Type;
use crate::{Env, Unevaluated};
use std::convert::TryInto;
use std::iter::Iterator;

pub const CONCAT: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let base: String = env.get_value("_")?.try_into()?;
    let other: String = env.get_value("other")?.try_into()?;
    Ok(Type::String(format!("{}{}", base, other)))
});

pub const SPLIT: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let base: String = env.get_value("_")?.try_into()?;
    let pat: String = env.get_value("pat")?.try_into()?;
    Ok(Type::List(
        base.split(&pat)
            .map(|s| Type::String(s.to_string()))
            .collect(),
    ))
});

#[test]
fn test_split() {
    use crate::eval::eval_source;
    use crate::token::Source;
    use std::str::FromStr;

    let ast = r#"
hoge: "HOGE,hoge",
hoge.split(",")"#;
    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    assert_eq!(
        result,
        Type::List(vec![
            Type::String("HOGE".to_string()),
            Type::String("hoge".to_string())
        ])
    );
}
