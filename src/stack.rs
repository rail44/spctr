use crate::{json, list, map, string};
use failure::format_err;
use pest::iterators::Pair;
use pest::Parser as PestParser;
use pest_derive::Parser as PestParser;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::iter::Iterator;
use std::rc::Rc;

#[derive(PestParser)]
#[grammar = "grammar.pest"]
struct Parser;

#[derive(Debug, Clone, PartialEq)]
pub struct Cmd(Vec<Op>);

#[derive(Debug, Clone, Default)]
pub struct Env {
    pub current_fn: Option<Function>,
    pub bind_map: Rc<RefCell<HashMap<String, Unevaluated>>>,
    pub evaluated_map: Rc<RefCell<HashMap<String, Value>>>,
    pub parent: Option<Box<Env>>,
}

impl PartialEq for Env {
    fn eq(&self, other: &Env) -> bool {
        self.bind_map == other.bind_map && self.parent == other.parent
    }
}

impl Env {
    pub fn new(bind_map: HashMap<String, Unevaluated>, evaluated_map: HashMap<String, Value>) -> Self {
        Self {
            bind_map: Rc::new(RefCell::new(bind_map)),
            evaluated_map: Rc::new(RefCell::new(evaluated_map)),
            ..Default::default()
        }
    }

    pub fn root() -> Self {
        let mut bind_map = HashMap::new();
        bind_map.insert(
            "Iterator".to_string(),
            Unevaluated::Cmd(get_stack(include_str!("iterator.spc")).unwrap()),
        );

        let mut evaluated_map = HashMap::new();
        evaluated_map.insert("List".to_string(), list::ListModule::get_value());
        evaluated_map.insert("Map".to_string(), map::MapModule::get_value());
        evaluated_map.insert("Json".to_string(), json::JsonModule::get_value());

        Self::new(bind_map, evaluated_map)
    }

    pub fn bind(&self, k: String, u: Unevaluated) {
        self.bind_map.borrow_mut().insert(k, u);
    }

    pub fn into_first_binding(self) -> Env {
        if self.bind_map.borrow().is_empty() && self.evaluated_map.borrow().is_empty() {
            return self.parent.unwrap().into_first_binding();
        }

        self
    }

    pub fn is_recursive_fn(&self, f: &Function) -> bool {
        self.get_current_fn().map_or(false, |current| current == f)
    }

    fn get_current_fn(&self) -> Option<&Function> {
        if let Some(ref current) = self.current_fn {
            return Some(current);
        }

        if let Some(ref p) = self.parent {
            return p.get_current_fn();
        }

        None
    }

    pub fn get_value(&mut self, name: &str) -> Result<Value, failure::Error> {
        if let Some(evaluated) = self.evaluated_map.borrow().get(name) {
            return Ok(evaluated.clone());
        }

        let binded = self.bind_map.borrow().get(name).cloned();
        if let Some(binded) = binded {
            let value = binded.eval(self)?;
            self.evaluated_map
                .borrow_mut()
                .insert(name.to_string(), value.clone());
            return Ok(value);
        }

        if let Some(ref mut p) = self.parent {
            return p.get_value(name);
        }

        Err(format_err!("Could not find bind `{}`", name))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    inner: Box<Env>,
    arg_name_list: Vec<String>,
    body: Unevaluated,
}

impl Function {
    fn call(&self, args: &[Value]) -> Result<Value, failure::Error> {
        let mut evaluated_map = HashMap::new();
        for (n, v) in self.arg_name_list.iter().zip(args) {
            evaluated_map.insert(n.to_string(), v.clone());
        }
        let mut child = Env::new(Default::default(), evaluated_map);
        child.parent = Some(self.inner.clone());
        child.current_fn = Some(self.clone());

        self.body.eval(&mut child)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Unevaluated {
    Cmd(Cmd),
    Native(fn(Env) -> Result<Value, failure::Error>),
}

impl Unevaluated {
    pub fn eval(&self, env: &mut Env) -> Result<Value, failure::Error> {
        match self {
            Unevaluated::Cmd(cmd) => Ok(eval(cmd, env)?.pop().unwrap()),
            Unevaluated::Native(f) => f(env.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    List(Vec<Value>),
    Map(Env),
    Function(Function),
    Boolean(bool),
    Null,
}

impl Function {
    pub fn new(inner: Env, arg_name_list: Vec<String>, body: Unevaluated) -> Self {
        Function {
            inner: Box::new(inner),
            arg_name_list,
            body,
        }
    }
}

impl Value {
    pub fn get_prop(&mut self, name: &str) -> Result<Value, failure::Error> {
        let mut evaluated_map = HashMap::new();
        evaluated_map.insert("_".to_string(), self.clone());
        let env = Env::new(Default::default(), evaluated_map);
        match self {
            Value::Map(env) => env.get_value(name),
            Value::String(_s) => match name {
                "concat" => {
                    Ok(Function::new(env, vec!["other".to_string()], string::CONCAT).into())
                }
                "split" => Ok(Function::new(env, vec!["pat".to_string()], string::SPLIT).into()),
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            Value::List(v) => match name {
                "count" => Ok(Value::Number(v.len() as f64)),
                "concat" => Ok(Function::new(env, vec!["other".to_string()], list::CONCAT).into()),
                "to_iter" => {
                    fn next(mut env: Env) -> Result<Value, failure::Error> {
                        let i: f64 = env.get_value("i")?.try_into()?;
                        let list: Vec<Value> = env.get_value("list")?.try_into()?;

                        Ok(list.get(i as usize).map_or(Value::Null, |v| {
                            let mut bind_map = HashMap::new();
                            bind_map.insert("next".to_string(), Unevaluated::Native(next));

                            let mut evaluated_map = HashMap::new();
                            evaluated_map.insert("i".to_string(), Value::Number(i + 1.0));

                            let mut new_env = Env::new(bind_map, evaluated_map);
                            new_env.parent = Some(Box::new(env.clone()));

                            Value::List(vec![Value::Map(new_env), v.clone()])
                        }))
                    }

                    let mut bind_map = HashMap::new();
                    bind_map.insert("next".to_string(), Unevaluated::Native(next));

                    let mut evaluated_map = HashMap::new();
                    evaluated_map.insert("i".to_string(), Value::Number(0.0));
                    evaluated_map.insert("list".to_string(), self.clone());

                    let mut new_env = Env::new(bind_map, evaluated_map);
                    new_env.parent = Some(Box::new(Env::root()));

                    let iterator_fn: Function = new_env.get_value("Iterator")?.try_into()?;
                    Ok(iterator_fn.call(&[Value::Map(new_env)])?)
                }
                _ => Err(format_err!("{} has no prop `{}`", self, name)),
            },
            _ => Err(format_err!("{} has no prop `{}`", self, name)),
        }
    }

    pub fn indexing(&self, n: i32) -> Result<Value, failure::Error> {
        match self {
            Value::List(vec) => Ok(vec[n as usize].clone()),
            _ => Err(format_err!("{} has no index {}", self, n)),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Value::Number(f) => write!(formatter, "{}", f),
            Value::String(s) => write!(formatter, "\"{}\"", s),
            Value::Map(env) => {
                let bind_map = env.clone().bind_map.borrow().clone();
                let pairs: Vec<String> = bind_map
                    .keys()
                    .map(|k| {
                        let v = env.clone().get_value(k).unwrap();
                        format!("\"{}\": {}", k, v)
                    })
                    .collect();
                write!(formatter, "{{{}}}", pairs.join(", "))
            }
            Value::List(vec) => {
                let v: Vec<String> = vec.iter().map(|e| format!("{}", e).to_string()).collect();
                write!(formatter, "[{}]", v.join(", "))
            }
            Value::Function(_) => write!(formatter, "[function]"),
            Value::Boolean(b) => write!(formatter, "{}", b),
            Value::Null => write!(formatter, "null"),
        }
    }
}

impl TryInto<String> for Value {
    type Error = failure::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        if let Value::String(s) = self {
            return Ok(s);
        }
        Err(format_err!("{} is not String", self))
    }
}

impl TryInto<Vec<Value>> for Value {
    type Error = failure::Error;

    fn try_into(self) -> Result<Vec<Value>, Self::Error> {
        if let Value::List(v) = self {
            return Ok(v);
        }
        Err(format_err!("{} is not List", self))
    }
}

impl TryInto<f64> for Value {
    type Error = failure::Error;

    fn try_into(self) -> Result<f64, Self::Error> {
        if let Value::Number(f) = self {
            return Ok(f);
        }
        Err(format_err!("{} is not Number", self))
    }
}

impl TryInto<Env> for Value {
    type Error = failure::Error;

    fn try_into(self) -> Result<Env, Self::Error> {
        if let Value::Map(env) = self {
            return Ok(env);
        }
        Err(format_err!("{} is not Map", self))
    }
}

impl TryInto<bool> for Value {
    type Error = failure::Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        if let Value::Boolean(b) = self {
            return Ok(b);
        }
        Err(format_err!("{} is not Boolean", self))
    }
}

impl TryInto<Function> for Value {
    type Error = failure::Error;

    fn try_into(self) -> Result<Function, Self::Error> {
        if let Value::Function(f) = self {
            return Ok(f);
        }
        Err(format_err!("{} is not Function", self))
    }
}

impl From<Function> for Value {
    fn from(f: Function) -> Value {
        Value::Function(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Surplus,
    Equal,
    NotEqual,
    Call(usize),
    BuildFunction(Vec<String>, Cmd),
    BuildList(usize),
    BuildMap,
    Fork,
    Exit,
    JumpUnless(usize),
    Jump(usize),
    SetBind(String, Cmd),
    GetBind(String),
    Access(String),
    Push(Value),
    Indexing,
}

impl From<Value> for Op {
    fn from(v: Value) -> Op {
        Op::Push(v)
    }
}

fn build_cmd(v: &mut Vec<Op>, pair: Pair<'_, Rule>) -> Result<(), failure::Error> {
    match pair.as_rule() {
        Rule::number => v.push(Value::Number(pair.as_str().parse()?).into()),
        Rule::add => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Add);
        }
        Rule::sub => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Sub);
        }
        Rule::mul => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Mul);
        }
        Rule::div => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Div);
        }
        Rule::surplus => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Surplus);
        }
        Rule::equal => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Equal);
        }
        Rule::not_equal => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::NotEqual);
        }
        Rule::identify => v.push(Op::GetBind(pair.as_str().to_string())),
        Rule::string_literal => {
            v.push(Op::Push(Value::String(
                pair.into_inner()
                    .next()
                    .unwrap()
                    .as_str()
                    .replace("\\\"", "\"")
                    .to_string(),
            )));
        }
        Rule::null => v.push(Op::Push(Value::Null)),
        Rule::function => {
            let mut inner = pair.into_inner();

            let mut body = vec![];
            build_cmd(&mut body, inner.next_back().unwrap())?;
            if let Some(i) = body.iter().rposition(|op| *op == Op::Exit) {
                body.truncate(i);
            }

            let arg_names = inner.map(|p| p.as_str().to_string()).collect();
            v.push(Op::BuildFunction(arg_names, Cmd(body)))
        }
        Rule::_if => {
            let mut inner = pair.into_inner();
            build_cmd(v, inner.next().unwrap())?;

            let mut cons = vec![];
            build_cmd(&mut cons, inner.next().unwrap())?;
            let alt_head = cons.len();

            let mut alt = vec![];
            build_cmd(&mut alt, inner.next().unwrap())?;
            let end = alt.len();

            v.push(Op::JumpUnless(alt_head + 2));
            v.append(&mut cons);

            v.push(Op::Jump(end + 1));
            v.append(&mut alt);
        }
        Rule::calling => {
            let inner = pair.into_inner();
            let len = inner.clone().count();
            build_cmd_from_iter(v, inner)?;
            v.push(Op::Call(len));
        }
        Rule::access => {
            v.push(Op::Access(
                pair.into_inner().next().unwrap().as_str().to_string(),
            ));
        }
        Rule::block => {
            v.push(Op::Fork);
            build_cmd_from_iter(v, pair.into_inner())?;
            if let Op::SetBind(_, _) = v.last().unwrap() {
                v.push(Op::BuildMap);
            }
            v.push(Op::Exit);
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
            let mut stack_vec = vec![];
            build_cmd_from_iter(&mut stack_vec, inner)?;
            v.push(Op::SetBind(name.to_string(), Cmd(stack_vec)));
        }
        Rule::body => {
            build_cmd_from_iter(v, pair.into_inner())?;
        }
        Rule::indexing => {
            build_cmd_from_iter(v, pair.into_inner())?;
            v.push(Op::Indexing);
        }
        Rule::list => {
            let inner = pair.into_inner();
            let len = inner.clone().count();
            build_cmd_from_iter(v, inner)?;
            v.push(Op::BuildList(len));
        }
        _ => return Err(format_err!("{:?}", pair)),
    };
    Ok(())
}

fn build_cmd_from_iter<'a, I: IntoIterator<Item = Pair<'a, Rule>>>(
    v: &mut Vec<Op>,
    iter: I,
) -> Result<(), failure::Error> {
    for p in iter {
        build_cmd(v, p)?;
    }
    Ok(())
}

pub fn get_stack(s: &str) -> Result<Cmd, failure::Error> {
    let mut v = vec![];
    build_cmd_from_iter(&mut v, Parser::parse(Rule::source, s)?)?;
    Ok(Cmd(v))
}

pub fn eval(cmd: &Cmd, env: &mut Env) -> Result<Vec<Value>, failure::Error> {
    let mut stack: Vec<Value> = vec![];

    let mut i = 0;
    while i < cmd.0.len() {
        match &cmd.0[i] {
            Op::Add => {
                let second: f64 = stack.pop().unwrap().try_into()?;
                let first: f64 = stack.pop().unwrap().try_into()?;
                stack.push(Value::Number(first + second))
            }
            Op::Sub => {
                let second: f64 = stack.pop().unwrap().try_into()?;
                let first: f64 = stack.pop().unwrap().try_into()?;
                stack.push(Value::Number(first - second));
            }
            Op::Div => {
                let second: f64 = stack.pop().unwrap().try_into()?;
                let first: f64 = stack.pop().unwrap().try_into()?;
                stack.push(Value::Number(first / second));
            }
            Op::Mul => {
                let second: f64 = stack.pop().unwrap().try_into()?;
                let first: f64 = stack.pop().unwrap().try_into()?;
                stack.push(Value::Number(first * second));
            }
            Op::Surplus => {
                let second: f64 = stack.pop().unwrap().try_into()?;
                let first: f64 = stack.pop().unwrap().try_into()?;
                stack.push(Value::Number(first % second));
            }
            Op::Equal => {
                let second = stack.pop().unwrap();
                let first = stack.pop().unwrap();
                stack.push(Value::Boolean(first == second));
            }
            Op::NotEqual => {
                let second = stack.pop().unwrap();
                let first = stack.pop().unwrap();
                stack.push(Value::Boolean(first != second));
            }
            Op::BuildFunction(arg_names, body) => {
                stack.push(
                    Function::new(
                        env.clone(),
                        arg_names.clone(),
                        Unevaluated::Cmd(body.clone()),
                    )
                    .into(),
                );
            }
            Op::BuildList(len) => {
                let list = Value::List(stack.split_off(stack.len() - len));
                stack.push(list);
            }
            Op::Fork => {
                let mut child = Env::default();
                child.parent = Some(Box::new(env.clone().into_first_binding()));
                *env = child.clone();
            }
            Op::Exit => {
                *env = *env.parent.as_ref().unwrap().clone();
            }
            Op::Call(len) => {
                let args = stack.split_off(stack.len() - len);
                let f: Function = stack.pop().unwrap().try_into()?;

                if cmd.0.len() == i + 1 && env.is_recursive_fn(&f) {
                    i = 0;
                    let mut evaluated_map = HashMap::new();
                    for (n, v) in f.arg_name_list.iter().zip(args) {
                        evaluated_map.insert(n.to_string(), v.clone());
                    }
                    *env = Env::new(Default::default(), evaluated_map);
                    env.parent = Some(f.inner);

                    continue;
                }

                stack.push(f.call(&args)?);
            }
            Op::SetBind(name, bind) => {
                env.bind(name.to_string(), Unevaluated::Cmd(bind.clone()));
            }
            Op::BuildMap => {
                stack.push(Value::Map(env.clone()));
            }
            Op::Access(name) => {
                let mut v = stack.pop().unwrap();
                stack.push(v.get_prop(&name)?);
            }
            Op::Indexing => {
                let index = stack.pop().unwrap();
                let mut v = stack.pop().unwrap();
                match index {
                    Value::String(s) => stack.push(v.get_prop(&s)?),
                    Value::Number(n) => stack.push(v.indexing(n as i32)?),
                    v => return Err(format_err!("{:?}", v)),
                }
            }
            Op::GetBind(name) => {
                stack.push(env.get_value(&name)?);
            }
            Op::Push(v) => {
                stack.push(v.clone());
            }
            Op::JumpUnless(j) => {
                let cond: bool = stack.pop().unwrap().try_into()?;
                if cond {
                    i += 1;
                    continue;
                }
                i += j;
                continue;
            }
            Op::Jump(j) => {
                i += j;
                continue;
            }
        }
        i += 1;
    }
    Ok(stack)
}
