use crate::Env;
use crate::types::{Type, NativeCallable};

#[derive(Debug, Clone, PartialEq)]
pub struct Concat(String);

impl Concat {
    pub fn new(s: String) -> Self {
        Concat(s)
    }
}

impl NativeCallable for Concat {
    fn comparator(&self) -> &str {
        &self.0
    }

    fn call(&self, _env: &mut Env, mut args: Vec<Type>) -> Type {
        if let Type::String(s) = args.pop().unwrap() {
            return Type::String(format!("{}{}", self.0, s));
        }
        panic!();
    }

    fn box_clone(&self) -> Box<dyn NativeCallable> {
        Box::new(self.clone())
    }
}
