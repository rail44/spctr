use pest::iterators::{Pair, Pairs};
use pest::Parser as PestParser;
use pest_derive::Parser as PestParser;
use std::iter::Iterator;
use std::collections::HashMap;
use std::convert::{TryInto};
use std::cell::RefCell;
use std::rc::Rc;
use failure::format_err;

#[derive(PestParser)]
#[grammar = "grammar.pest"]
struct Parser;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Env<'a> {
    bind_map: Rc<RefCell<HashMap<String, Pairs<'a, Rule>>>>,
    evaluated_map: Rc<RefCell<HashMap<String, Value<'a>>>>,
    parents: Vec<Env<'a>>,
}

impl<'a> Env<'a> {
    fn new(bind_map: HashMap<String, Pairs<'a, Rule>>, evaluated_map: HashMap<String, Value<'a>>) -> Self {
        Env::<'a> {
            bind_map: Rc::new(RefCell::new(bind_map)),
            evaluated_map: Rc::new(RefCell::new(evaluated_map)),
            parents: vec![],
        }
    }

    pub fn root() -> Self {
        let mut evaluated_map = HashMap::new();
        // evaluated_map.insert("List".to_string(), list::ListModule::get_value());
        // evaluated_map.insert("Map".to_string(), map::MapModule::get_value());
        // evaluated_map.insert("Json".to_string(), json::JsonModule::get_value());

        Env {
            evaluated_map: Rc::new(RefCell::new(evaluated_map)),
            ..Default::default()
        }
    }

    fn insert(&self, name: String, s: Pairs<'a, Rule>) {
        self.bind_map.borrow_mut().insert(name, s);
    }

    fn insert_evaluated(&self, name: String, s: Value<'a>) {
        self.evaluated_map.borrow_mut().insert(name, s);
    }

    fn get_value(&self, name: &str) -> Result<Value<'a>, failure::Error> {
        if let Some(evaluated) = self.evaluated_map.borrow().get(name) {
            return Ok(evaluated.clone());
        }

        if let Some(binded) = { self.bind_map.borrow().get(name).clone() } {
            let mut stack = vec![];
            eval_pairs(&mut stack, self, &mut binded.clone())?;
            let value = stack.pop().unwrap();
            self.evaluated_map
                .borrow_mut()
                .insert(name.to_string(), value.clone());
            return Ok(value);
        }

        for p in self.parents.iter() {
            match p.get_value(name) {
                Ok(v) => return Ok(v),
                Err(_) => (),
            }
        }

        Err(format_err!("Could not find bind `{}`", name))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Number(f64),
    String(String),
    List(Vec<Value<'a>>),
    Map(Env<'a>),
    Function(Env<'a>, Vec<String>, Pairs<'a, Rule>),
    Boolean(bool),
    Null,
}

impl<'a> std::fmt::Display for Value<'a> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Value::Number(f) => write!(formatter, "{}", f),
            Value::String(s) => write!(formatter, "\"{}\"", s),
            Value::Map(env) => {
                let bind_map = env.clone().bind_map.borrow().clone();
                let pairs: Vec<String> = bind_map
                    .keys()
                    .map(|k| {
                        let v = env.get_value(k).unwrap();
                        format!("\"{}\": {}", k, v)
                    })
                    .collect();
                write!(formatter, "{{{}}}", pairs.join(", "))
            }
            Value::List(vec) => {
                let v: Vec<String> = vec.iter().map(|e| format!("{}", e).to_string()).collect();
                write!(formatter, "[{}]", v.join(", "))
            }
            Value::Function(_, _, _) => write!(formatter, "[function]"),
            Value::Boolean(b) => write!(formatter, "{}", b),
            Value::Null => write!(formatter, "null"),
        }
    }
}

impl TryInto<String> for Value<'_> {
    type Error = failure::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        if let Value::String(s) = self {
            return Ok(s);
        }
        Err(format_err!("{} is not String", self))
    }
}

impl<'a> TryInto<Vec<Value<'a>>> for Value<'a> {
    type Error = failure::Error;

    fn try_into(self) -> Result<Vec<Value<'a>>, Self::Error> {
        if let Value::List(v) = self {
            return Ok(v);
        }
        Err(format_err!("{} is not List", self))
    }
}

impl TryInto<f64> for Value<'_> {
    type Error = failure::Error;

    fn try_into(self) -> Result<f64, Self::Error> {
        if let Value::Number(f) = self {
            return Ok(f);
        }
        Err(format_err!("{} is not Number", self))
    }
}

impl<'a> TryInto<Env<'a>> for Value<'a> {
    type Error = failure::Error;

    fn try_into(self) -> Result<Env<'a>, Self::Error> {
        if let Value::Map(env) = self {
            return Ok(env);
        }
        Err(format_err!("{} is not Map", self))
    }
}

impl TryInto<bool> for Value<'_> {
    type Error = failure::Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        if let Value::Boolean(b) = self {
            return Ok(b);
        }
        Err(format_err!("{} is not Boolean", self))
    }
}

fn eval_pair<'a>(stack: &mut Vec<Value<'a>>, env: &Env<'a>, pair: Pair<'a, Rule>) -> Result<(), failure::Error> {
    match pair.as_rule() {
        Rule::number => stack.push(Value::Number(pair.as_str().parse()?)),
        Rule::add => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Number(first + second));
        }
        Rule::sub => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Number(first - second));
        }
        Rule::mul => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Number(first * second));
        }
        Rule::div => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Number(first / second));
        }
        Rule::surplus => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Number(first % second));
        }
        Rule::equal => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Boolean(first == second));
        }
        Rule::not_equal => {
            eval_pairs(stack, env, pair.into_inner())?;
            let second: f64 = stack.pop().unwrap().try_into()?;
            let first: f64 = stack.pop().unwrap().try_into()?;
            stack.push(Value::Boolean(first != second));
        }
        Rule::identify => stack.push(env.get_value(pair.as_str())?),
        Rule::function => {
            let mut inner = pair.into_inner();
            let body = inner.next_back().unwrap().into_inner();
            let arg_names: Vec<String> = inner.map(|p| p.as_str().to_string()).collect();
            stack.push(Value::Function(env.clone(), arg_names, body));
        }
        Rule::primary => {
            eval_pairs(stack, env, pair.into_inner())?;
        }
        Rule::calling => {
            match  stack.pop().unwrap() {
                Value::Function(f_env, arg_names, body) => {
                    let mut inner = pair.into_inner();
                    for arg_name in arg_names {
                        let mut child_stack = vec![];
                        eval_pair(&mut child_stack, env, inner.next().unwrap())?;
                        f_env.insert_evaluated(arg_name, child_stack.pop().unwrap())
                    }

                    let mut child_stack = vec![];
                    eval_pairs(&mut child_stack, &f_env, body)?;

                    stack.push(child_stack.pop().unwrap());
                }
                _ => return Err(format_err!("{:?}", pair)),
            }
        }
        Rule::access => {
            let map: Env = stack.pop().unwrap().try_into()?;
            stack.push(map.get_value(pair.into_inner().next().unwrap().as_str())?);
        }
        Rule::block => {
            let mut child: Env = Default::default();
            child.parents.push(env.clone());
            let mut child_stack = vec![];
            eval_pairs(&mut child_stack, &mut child, pair.into_inner())?;

            if child_stack.is_empty() {
                stack.push(Value::Map(child));
                return Ok(());
            }
            stack.push(child_stack.pop().unwrap());
        }
        Rule::spread => {
            unimplemented!();
        }
        Rule::bind => {
            let mut inner = pair.into_inner();
            let ident = inner.next().unwrap();
            let name = match ident.as_rule() {
                Rule::identify => ident.as_str(),
                Rule::string_literal => ident.into_inner().next().unwrap().as_str(),
                _ => panic!(),
            };
            env.insert(name.to_string(), inner);
        }
        Rule::body => {
            eval_pairs(stack, env, pair.into_inner())?;
        }
        _ => return Err(format_err!("{:?}", pair)),
    };
    Ok(())
}

fn eval_pairs<'a, I: IntoIterator<Item=Pair<'a, Rule>>>(stack: &mut Vec<Value<'a>>, env: &Env<'a>, iter: I) -> Result<(), failure::Error> {
    for p in iter {
        eval_pair(stack, env, p)?;
    }
    Ok(())
}

pub fn eval<'a>(s: &'a str) -> Result<Value<'a>, failure::Error> {
    let mut stack = vec![];
    eval_pairs(&mut stack, &Default::default(), Parser::parse(Rule::source, s)?)?;
    Ok(stack.pop().unwrap())
}
