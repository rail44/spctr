use crate::symbol::{display, Symbol};
use std::collections::{HashMap, HashSet};
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
    Record(Vec<(Symbol, Type)>),
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
            Type::Record(fields) => {
                Type::Record(fields.iter().map(|(n, t)| (*n, t.apply(s))).collect())
            }
            _ => self.clone(),
        }
    }

    pub fn contains(&self, var: TypeVar) -> bool {
        match self {
            Type::Var(v) => *v == var,
            Type::Fn(args, ret) => ret.contains(var) || args.iter().any(|a| a.contains(var)),
            Type::List(t) => t.contains(var),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains(var)),
            _ => false,
        }
    }

    pub fn free_vars(&self, set: &mut HashSet<TypeVar>) {
        match self {
            Type::Var(v) => {
                set.insert(*v);
            }
            Type::Fn(args, ret) => {
                for a in args {
                    a.free_vars(set);
                }
                ret.free_vars(set);
            }
            Type::List(t) => t.free_vars(set),
            Type::Record(fields) => {
                for (_, t) in fields {
                    t.free_vars(set);
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scheme {
    pub vars: Vec<TypeVar>,
    pub ty: Type,
}

impl Scheme {
    pub fn mono(ty: Type) -> Self {
        Scheme {
            vars: Vec::new(),
            ty,
        }
    }

    pub fn free_vars(&self, set: &mut HashSet<TypeVar>) {
        let mut tmp = HashSet::new();
        self.ty.free_vars(&mut tmp);
        for v in &self.vars {
            tmp.remove(v);
        }
        set.extend(tmp);
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
            Type::Record(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(n, t)| format!("{}: {}", display(*n), t))
                    .collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
        }
    }
}
