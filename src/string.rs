use crate::types::Type;
use std::iter::Iterator;

pub fn concat(receiver: Type, args: Vec<Type>) -> Type {
    if let Type::String(base) = receiver {
        let rights: String = args
            .into_iter()
            .map(|s| {
                if let Type::String(s) = s {
                    return s;
                }
                panic!();
            })
            .collect();
        return Type::String(format!("{}{}", base, rights));
    }
    panic!();
}

pub fn split(receiver: Type, mut args: Vec<Type>) -> Type {
    if let Type::String(base) = receiver {
        if let Type::String(pat) = args.remove(0) {
            return Type::List(
                base.split(&pat)
                    .map(|s| Type::String(s.to_string()))
                    .collect(),
            );
        }
        panic!();
    }
    panic!();
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
    let result = eval_source(source, &mut Default::default());
    assert_eq!(
        result,
        Type::List(vec![
            Type::String("HOGE".to_string()),
            Type::String("hoge".to_string())
        ])
    );
}
