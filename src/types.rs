use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeVar(pub u32);

#[derive(Clone, Debug)]
pub enum Type {
    Number,
    String,
    Bool,
    Null,
    Any,
    Var(TypeVar),
    Fn(Vec<Type>, Box<Type>),
    List(Box<Type>),
}

pub type Subst = HashMap<TypeVar, Type>;

impl Type {
    pub fn apply(&self, s: &Subst) -> Type {
        match self {
            Type::Var(v) => match s.get(v) {
                Some(t) => t.apply(s),
                None => self.clone(),
            },
            Type::Fn(args, ret) => Type::Fn(
                args.iter().map(|a| a.apply(s)).collect(),
                Box::new(ret.apply(s)),
            ),
            Type::List(t) => Type::List(Box::new(t.apply(s))),
            _ => self.clone(),
        }
    }

    pub fn contains(&self, var: TypeVar) -> bool {
        match self {
            Type::Var(v) => *v == var,
            Type::Fn(args, ret) => ret.contains(var) || args.iter().any(|a| a.contains(var)),
            Type::List(t) => t.contains(var),
            _ => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Bool => write!(f, "bool"),
            Type::Null => write!(f, "null"),
            Type::Any => write!(f, "any"),
            Type::Var(v) => write!(f, "?{}", v.0),
            Type::Fn(args, ret) => {
                let args_str: Vec<String> = args.iter().map(|t| t.to_string()).collect();
                write!(f, "({}) -> {}", args_str.join(", "), ret)
            }
            Type::List(t) => write!(f, "list<{}>", t),
        }
    }
}
