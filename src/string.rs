use crate::types::Type;

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
