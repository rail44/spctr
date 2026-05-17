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
    /// Module-like value with potentially polymorphic field schemes.
    /// Used for builtin modules (List, String, Iterator…); each field can
    /// be instantiated independently when accessed.
    Module(Vec<(Symbol, Scheme)>),
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
            Type::Module(fields) => Type::Module(
                fields
                    .iter()
                    .map(|(n, sch)| {
                        // Don't substitute quantified vars
                        let mut s_filtered = s.clone();
                        for v in &sch.vars {
                            s_filtered.remove(v);
                        }
                        (
                            *n,
                            Scheme {
                                vars: sch.vars.clone(),
                                ty: sch.ty.apply(&s_filtered),
                            },
                        )
                    })
                    .collect(),
            ),
            _ => self.clone(),
        }
    }

    pub fn contains(&self, var: TypeVar) -> bool {
        match self {
            Type::Var(v) => *v == var,
            Type::Fn(args, ret) => ret.contains(var) || args.iter().any(|a| a.contains(var)),
            Type::List(t) => t.contains(var),
            Type::Record(fields) => fields.iter().any(|(_, t)| t.contains(var)),
            Type::Module(fields) => fields.iter().any(|(_, sch)| {
                !sch.vars.contains(&var) && sch.ty.contains(var)
            }),
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
            Type::Module(fields) => {
                for (_, sch) in fields {
                    sch.free_vars(set);
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

/// Maps raw `TypeVar` ids to human-friendly names (α, β, γ, …, ω, α1, β1, …).
/// Names are assigned in first-encounter order so the same id always renders
/// to the same name within a single rendering pass.
#[derive(Default)]
pub struct Renamer {
    map: HashMap<TypeVar, String>,
}

impl Renamer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&mut self, v: TypeVar) -> String {
        if let Some(s) = self.map.get(&v) {
            return s.clone();
        }
        let n = self.map.len();
        let s = greek_name(n);
        self.map.insert(v, s.clone());
        s
    }
}

fn greek_name(n: usize) -> String {
    // α..ω is 24 letters (U+03B1..U+03C9, skipping U+03C2 final-sigma).
    const LETTERS: [char; 24] = [
        'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'σ',
        'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
    ];
    let letter = LETTERS[n % LETTERS.len()];
    let cycle = n / LETTERS.len();
    if cycle == 0 {
        letter.to_string()
    } else {
        format!("{}{}", letter, cycle)
    }
}

fn fmt_type(ty: &Type, ren: &mut Renamer, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match ty {
        Type::Number => f.write_str("number"),
        Type::String => f.write_str("string"),
        Type::Bool => f.write_str("bool"),
        Type::Null => f.write_str("null"),
        Type::Any => f.write_str("any"),
        Type::Var(v) => f.write_str(&ren.name(*v)),
        Type::Fn(args, ret) => {
            f.write_str("(")?;
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                fmt_type(a, ren, f)?;
            }
            f.write_str(") -> ")?;
            fmt_type(ret, ren, f)
        }
        Type::List(t) => {
            f.write_str("list<")?;
            fmt_type(t, ren, f)?;
            f.write_str(">")
        }
        Type::Record(fields) => {
            f.write_str("{")?;
            for (i, (n, t)) in fields.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}: ", display(*n))?;
                fmt_type(t, ren, f)?;
            }
            f.write_str("}")
        }
        Type::Module(fields) => {
            f.write_str("module{")?;
            for (i, (n, sch)) in fields.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}: ", display(*n))?;
                fmt_scheme(sch, ren, f)?;
            }
            f.write_str("}")
        }
    }
}

fn fmt_scheme(sch: &Scheme, ren: &mut Renamer, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if sch.vars.is_empty() {
        return fmt_type(&sch.ty, ren, f);
    }
    f.write_str("forall ")?;
    for (i, v) in sch.vars.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        f.write_str(&ren.name(*v))?;
    }
    f.write_str(". ")?;
    fmt_type(&sch.ty, ren, f)
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ren = Renamer::new();
        fmt_type(self, &mut ren, f)
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ren = Renamer::new();
        // Register quantified vars first so they appear as α, β, …
        // before any free vars in the body.
        for v in &self.vars {
            ren.name(*v);
        }
        fmt_scheme(self, &mut ren, f)
    }
}

/// Render two types under a shared renaming map so that the same `TypeVar`
/// resolves to the same name on both sides — useful for "X vs Y" diagnostics.
pub fn pretty_pair(a: &Type, b: &Type) -> (String, String) {
    use std::fmt::Write;
    struct W<'a> {
        ty: &'a Type,
        ren: std::cell::RefCell<&'a mut Renamer>,
    }
    impl<'a> fmt::Display for W<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt_type(self.ty, &mut self.ren.borrow_mut(), f)
        }
    }
    let mut ren = Renamer::new();
    let mut sa = String::new();
    let mut sb = String::new();
    write!(
        sa,
        "{}",
        W {
            ty: a,
            ren: std::cell::RefCell::new(&mut ren)
        }
    )
    .unwrap();
    write!(
        sb,
        "{}",
        W {
            ty: b,
            ren: std::cell::RefCell::new(&mut ren)
        }
    )
    .unwrap();
    (sa, sb)
}
