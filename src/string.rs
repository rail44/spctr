use crate::{Env, Type, Native};

#[derive(Debug, Clone, PartialEq)]
pub struct Concat(String);

impl Concat {
    pub fn new(s: String) -> Self {
        Concat(s)
    }
}

impl Native for Concat {
    fn comparator(&self) -> &str {
        &self.0
    }

    fn get_prop(self, env: &mut Env, name: &str) -> Type {
        unimplemented!();
    }

    fn call(&self, mut args: Vec<Type>) -> Type {
        if let Type::String(s) = args.pop().unwrap() {
            return Type::String(format!("{}{}", self.0, s))
        }
        panic!();
    }
}
