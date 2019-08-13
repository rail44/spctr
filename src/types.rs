use std::collections::HashMap;
use std::rc::Rc;
use std::fmt::Debug;
use std::cell::RefCell;
use std::any::Any;
use crate::{token, Env, list, string};
use crate::eval::Evaluable;

#[derive(Debug)]
pub struct BoxedNative(Box<dyn Native>);

impl BoxedNative {
    pub fn new<N: Native>(n: N) -> Self {
        BoxedNative(Box::new(n))
    }

    pub fn get_prop(&self, env: &mut Env, name: &str) -> Type {
        self.0.get_prop(env, name)
    }
}

pub trait Native: 'static + Debug {
    fn get_prop(&self, env: &mut Env, name: &str) -> Type;
    fn comparator(&self) -> &str;
    fn box_clone(&self) -> Box<dyn Native>;
}

impl Clone for BoxedNative {
    fn clone(&self) -> Self {
        BoxedNative(self.0.box_clone())
    }
}

impl From<BoxedNative> for Type {
    fn from(n: BoxedNative) -> Type {
        Type::Native(n)
    }
}

impl PartialEq for BoxedNative {
    fn eq(&self, other: &Self) -> bool {
        self.0.type_id() == other.0.type_id() && self.0.comparator() == other.0.comparator()
    }
}

#[derive(Debug)]
pub struct BoxedNativeCallable(Box<dyn NativeCallable>);

impl BoxedNativeCallable {
    pub fn new<N: NativeCallable>(n: N) -> Self {
        BoxedNativeCallable(Box::new(n))
    }

    pub fn call(&self, env: &mut Env, args: Vec<Type>) -> Type {
        self.0.call(env, args)
    }
}

pub trait NativeCallable: 'static + Debug {
    fn call(&self, env: &mut Env, args: Vec<Type>) -> Type;
    fn comparator(&self) -> &str;
    fn box_clone(&self) -> Box<dyn NativeCallable>;
}

impl Clone for BoxedNativeCallable {
    fn clone(&self) -> Self {
        BoxedNativeCallable(self.0.box_clone())
    }
}

impl PartialEq for BoxedNativeCallable {
    fn eq(&self, other: &Self) -> bool {
        self.0.type_id() == other.0.type_id() && self.0.comparator() == other.0.comparator()
    }
}

impl From<BoxedNativeCallable> for Type {
    fn from(n: BoxedNativeCallable) -> Type {
        Type::NativeCallable(n)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number(f64),
    String(String),
    List(Vec<Type>),
    Map(HashMap<String, token::Expression>),
    Function(Env, Vec<String>, Box<token::Expression>),
    Boolean(bool),
    Native(BoxedNative),
    NativeCallable(BoxedNativeCallable),
}

impl Type {
    pub fn get_prop(&self, env: &mut Env, name: &str) -> Type {
        match self {
            Type::Map(map) => {
                let mut child = Env {
                    binds: map.clone(),
                    evaluated: HashMap::new(),
                    parent: Some(Rc::new(RefCell::new(env.clone()))),
                };
                child.get_value(name)
            }
            Type::List(v) => match name {
                "map" => BoxedNativeCallable::new(list::Map::new(v.clone())).into(),
                _ => panic!(),
            },
            Type::String(s) => match name {
                "concat" => BoxedNativeCallable::new(string::Concat::new(s.clone())).into(),
                _ => panic!(),
            },
            Type::Native(n) => n.get_prop(env, name),
            _ => unreachable!(),
        }
    }

    pub fn call(self, env: &mut Env, args: Vec<Type>) -> Type {
        match self {
            Type::Function(inner_env, arg_names, expression) => {
                let mut evaluated = HashMap::new();
                for (v, n) in args.into_iter().zip(arg_names.iter()) {
                    evaluated.insert(n.clone(), v);
                }
                let mut env = Env {
                    binds: HashMap::new(),
                    evaluated,
                    parent: Some(Rc::new(RefCell::new(inner_env))),
                };
                expression.eval(&mut env)
            }
            Type::NativeCallable(n) => n.call(env, args),
            _ => unreachable!(),
        }
    }
}
