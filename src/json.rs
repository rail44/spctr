use crate::eval::eval_source;
use crate::token::Source;
use crate::types::Type;
use crate::Env;
use crate::Unevaluated;
use std::convert::TryInto;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonModule;

impl JsonModule {
    pub fn get_value() -> Type {
        let env = Env::default();
        env.insert(
            "parse".to_string(),
            Type::Function(env.clone(), vec!["s".to_string()], PARSE),
        );

        Type::Map(env)
    }
}

impl std::fmt::Display for JsonModule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JsonModule")
    }
}

pub const PARSE: Unevaluated = Unevaluated::Native(|env: Env| -> Result<Type, failure::Error> {
    let s: String = env.get_value("s")?.try_into()?;
    eval_source(Source::from_str(&s).unwrap(), &mut Default::default())
});

#[test]
fn test_access_json() {
    let ast = r#"
json_string: "{\"hoge\": [1, 2, null]}",
json: Json.parse(json_string),
json.hoge[2]"#;

    let source = Source::from_str(ast).unwrap();
    let result = eval_source(source, &mut Env::root()).unwrap();
    println!("{}", result);
    assert!(result == Type::Null);
}
