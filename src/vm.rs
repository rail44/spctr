use crate::stack::Cmd;
use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::mem;
use std::rc::Rc;

#[derive(Clone)]
pub struct ForeignFunction(pub Rc<dyn Fn(&Scope, Vec<Value>) -> Value>);

impl fmt::Debug for ForeignFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[native function]")
    }
}

#[derive(Clone, Debug)]
pub struct Value {
    pub primitive: Primitive,
    pub field: Rc<HashMap<String, usize>>,
    scope: Scope,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.primitive {
            Primitive::Number(n) => write!(f, "{}", n),
            Primitive::String(ref s) => write!(f, "\"{}\"", s),
            Primitive::Bool(b) => write!(f, "{}", b),
            Primitive::Function(_) => write!(f, "[function]"),
            Primitive::List(ref v) => {
                let fmt_values: Vec<_> = v.iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", fmt_values.join(", "))
            }
            Primitive::Null => write!(f, "null"),
            Primitive::Block(_) => {
                let mut vm = VM::new();
                vm.scope = self.scope.clone();
                let fmt_entries: Vec<_> = self
                    .field
                    .iter()
                    .map(|(k, v)| {
                        let v = vm.run(&[Cmd::Load(*v, 0)]).unwrap();
                        format!("{}: {}", k, v)
                    })
                    .collect();
                write!(f, "{{{}}}", fmt_entries.join(", "))
            }
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.primitive == other.primitive
    }
}

#[derive(Clone, Debug)]
pub enum Primitive {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Function(Function),
    List(Rc<Vec<Value>>),
    Null,
    Block(usize),
}

impl PartialEq for Primitive {
    fn eq(&self, other: &Self) -> bool {
        use Primitive::*;
        match (self, other) {
            (Number(a), Number(b)) => (a - b).abs() < f64::EPSILON,
            (Null, Null) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Function {
    Native(usize, Scope),
    Foreign(ForeignFunction),
}

impl Value {
    pub fn number(f: f64) -> Value {
        Value {
            primitive: Primitive::Number(f),
            field: Rc::new(HashMap::new()),
            scope: Scope(None),
        }
    }

    pub fn null() -> Value {
        Value {
            primitive: Primitive::Null,
            field: Rc::new(HashMap::new()),
            scope: Scope(None),
        }
    }

    pub fn into_number(self) -> Result<f64> {
        match self.primitive {
            Primitive::Number(n) => Ok(n),
            _ => Err(anyhow!("not number")),
        }
    }

    pub fn bool(b: bool) -> Value {
        Value {
            primitive: Primitive::Bool(b),
            field: Rc::new(HashMap::new()),
            scope: Scope(None),
        }
    }

    pub fn into_bool(self) -> Result<bool> {
        match self.primitive {
            Primitive::Bool(b) => Ok(b),
            _ => Err(anyhow!("not bool")),
        }
    }

    pub fn function(f: Function) -> Value {
        Value {
            primitive: Primitive::Function(f),
            field: Rc::new(HashMap::new()),
            scope: Scope(None),
        }
    }

    pub fn into_function(self) -> Result<Function> {
        match self.primitive {
            Primitive::Function(func) => Ok(func),
            _ => Err(anyhow!("{:?} is not function", self.primitive)),
        }
    }

    pub fn string(v: Rc<String>) -> Value {
        let mut field = HashMap::new();
        field.insert("concat".to_string(), 0_usize);
        let mut binds = Vec::new();

        let cloned = v.clone();
        binds.push(Rc::new(RefCell::new(Bind::Evalueated(Value::function(
            Function::Foreign(ForeignFunction(Rc::new(move |_, mut args| {
                let dst = args.pop().unwrap().into_string().unwrap();
                let v = format!("{}{}", v, dst);
                Value::string(Rc::new(v))
            }))),
        )))));

        let mut scope = Scope(None);
        scope.push(binds);

        Value {
            primitive: Primitive::String(cloned),
            field: Rc::new(field),
            scope,
        }
    }

    pub fn into_string(self) -> Result<Rc<String>> {
        match self.primitive {
            Primitive::String(s) => Ok(s),
            _ => Err(anyhow!("{:?} is not string", self.primitive)),
        }
    }

    pub fn block(i: usize, field: Rc<HashMap<String, usize>>, scope: Scope) -> Value {
        Value {
            primitive: Primitive::Block(i),
            field,
            scope,
        }
    }

    pub fn into_block(self) -> Result<(usize, Rc<HashMap<String, usize>>, Scope)> {
        match self.primitive {
            Primitive::Block(addr) => Ok((addr, self.field, self.scope)),
            _ => Err(anyhow!("{:?} is not block", self.primitive)),
        }
    }

    pub fn list(v: Rc<Vec<Value>>) -> Value {
        let mut field = HashMap::new();
        field.insert("concat".to_string(), 0_usize);
        field.insert("to_iter".to_string(), 1_usize);
        let mut binds = Vec::new();

        let cloned = v.clone();
        binds.push(Rc::new(RefCell::new(Bind::Evalueated(Value::function(
            Function::Foreign(ForeignFunction(Rc::new(move |_, mut args| {
                let mut v = (*v).clone();
                let dst = args.pop().unwrap().into_list().unwrap();
                v.append(&mut (*dst).clone());
                Value::list(Rc::new(v))
            }))),
        )))));
        let v = cloned;

        let cloned = v.clone();
        binds.push(Rc::new(RefCell::new(Bind::Evalueated(Value::function(
            Function::Foreign(ForeignFunction(Rc::new(move |_, mut args| {
                let mut v = (*v).clone();
                let dst = args.pop().unwrap().into_list().unwrap();
                v.append(&mut (*dst).clone());
                Value::list(Rc::new(v))
            }))),
        )))));
        let v = cloned;

        let mut scope = Scope(None);
        scope.push(binds);

        Value {
            primitive: Primitive::List(v),
            field: Rc::new(field),
            scope,
        }
    }

    pub fn into_list(self) -> Result<Rc<Vec<Value>>> {
        match self.primitive {
            Primitive::List(v) => Ok(v),
            _ => Err(anyhow!("{:?} is not list", self.primitive)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Bind {
    Cmd(usize),
    Evalueated(Value),
}

#[derive(Clone, Debug)]
pub struct Scope(Option<Rc<(Binds, Scope)>>);

type Binds = Vec<Rc<RefCell<Bind>>>;

impl Scope {
    fn push(&mut self, binds: Binds) {
        self.0 = Some(Rc::new((binds, Scope(self.0.take()))));
    }

    fn pop(&mut self) -> Binds {
        let rc = self.0.take().unwrap();
        let (head, tail) = Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone());
        *self = tail;
        head
    }

    fn nth_parent(&self, n: usize) -> &Scope {
        if n == 0 {
            return self;
        }
        let p: &Scope = &self.0.as_ref().unwrap().1;
        p.nth_parent(n - 1)
    }
}

pub fn run(program: &[Cmd]) -> Result<Value> {
    let mut vm = VM::new();
    vm.run(program)
}

struct VM {
    scope: Scope,
    call_stack: Vec<(usize, Scope)>,
}

impl VM {
    fn new() -> VM {
        let scope: Scope = Scope(None);
        VM {
            scope,
            call_stack: Vec::new(),
        }
    }

    fn run(&mut self, program: &[Cmd]) -> Result<Value> {
        let mut i: usize = 0;
        let mut stack: Vec<Value> = Vec::new();
        let len = program.len();
        while len > i {
            use Cmd::*;
            match program[i] {
                Add => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l + r));
                    i += 1;
                    continue;
                }
                Sub => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l - r));
                    i += 1;
                    continue;
                }
                Mul => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l * r));
                    i += 1;
                    continue;
                }
                Div => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l / r));
                    i += 1;
                    continue;
                }
                Surplus => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l % r));
                    i += 1;
                    continue;
                }
                Equal => {
                    let r = stack.pop().unwrap();
                    let l = stack.pop().unwrap();
                    stack.push(Value::bool(r == l));
                    i += 1;
                    continue;
                }
                Not => {
                    let b = stack.pop().unwrap().into_bool()?;
                    stack.push(Value::bool(!b));
                    i += 1;
                    continue;
                }
                Cmd::GreaterThan => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::bool(l > r));
                    i += 1;
                    continue;
                }
                Cmd::LessThan => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::bool(l < r));
                    i += 1;
                    continue;
                }
                NumberConst(n) => {
                    stack.push(Value::number(n));
                    i += 1;
                    continue;
                }
                StringConst(ref s) => {
                    stack.push(Value::string(s.clone()));
                    i += 1;
                    continue;
                }
                ConstructList(size) => {
                    let mut vec = Vec::new();
                    for _ in 0..size {
                        let v = stack.pop().unwrap();
                        vec.push(v);
                    }
                    vec.reverse();
                    stack.push(Value::list(Rc::new(vec)));
                    i += 1;
                    continue;
                }
                NullConst => {
                    stack.push(Value::null());
                    i += 1;
                    continue;
                }
                Block(ref def_addrs) => {
                    let mut binds = Vec::new();
                    let mut body_base = i + 1;
                    for addr in def_addrs.iter() {
                        binds.push(Rc::new(RefCell::new(Bind::Cmd(body_base))));
                        body_base += addr;
                    }
                    i = body_base;

                    self.scope.push(binds);

                    continue;
                }
                ExitScope => {
                    self.scope.pop();
                    i += 1;
                    continue;
                }
                JumpRel(n) => {
                    i += n;
                    continue;
                }
                JumpRelIf(n) => {
                    let cond = stack.pop().unwrap().into_bool()?;
                    if cond {
                        i += n;
                        continue;
                    }
                    i += 1;
                    continue;
                }
                Load(n, depth) => {
                    let scope = self.scope.nth_parent(depth).clone();
                    let binds: &Binds = scope.0.as_ref().unwrap().0.as_ref();

                    let bind = binds.get(n).unwrap();
                    let inner = bind.borrow().clone();
                    match inner {
                        Bind::Evalueated(v) => {
                            stack.push(v);
                            i += 1;
                            continue;
                        }
                        Bind::Cmd(addr) => {
                            let ret_i = i + 1;
                            let ret_scope = mem::replace(&mut self.scope, scope.clone());

                            self.call_stack.push((ret_i, ret_scope));
                            i = addr;
                            continue;
                        }
                    };
                }
                Return => {
                    let (ret_i, ret_scope) = self.call_stack.pop().unwrap();
                    i = ret_i;
                    self.scope = ret_scope;
                    continue;
                }
                Store(n) => {
                    let v = stack.pop().unwrap();
                    let binds: &Binds = self.scope.0.as_ref().unwrap().0.as_ref();

                    let bind = binds.get(n).unwrap();
                    *bind.borrow_mut() = Bind::Evalueated(v.clone());
                    stack.push(v);

                    i += 1;
                    continue;
                }
                ConstructFunction(len) => {
                    let body_base = i + 1;
                    stack.push(Value::function(Function::Native(
                        body_base,
                        self.scope.clone(),
                    )));
                    i = body_base + len;
                    continue;
                }
                ForeignFunction(ref func) => {
                    stack.push(Value::function(Function::Foreign(func.clone())));
                    i += 1;
                    continue;
                }
                ConstructBlock(len, ref map) => {
                    stack.push(Value::block(i + 1, map.clone(), self.scope.clone()));
                    i += 1 + len;
                    continue;
                }
                Call(arg_len) => {
                    let len = stack.len() - arg_len;
                    let args = stack.split_off(len);

                    match stack.pop().unwrap().into_function()? {
                        Function::Native(addr, closure_scope) => {
                            let ret_scope = mem::replace(&mut self.scope, closure_scope);

                            let mut defs = Vec::new();
                            for arg in args {
                                defs.push(Rc::new(RefCell::new(Bind::Evalueated(arg))));
                            }
                            self.scope.push(defs);

                            self.call_stack.push((i + 1, ret_scope));
                            i = addr;
                            continue;
                        }
                        Function::Foreign(func) => {
                            stack.push(func.0(&self.scope, args));
                            i += 1;
                            continue;
                        }
                    }
                }
                Access => {
                    let name = stack.pop().unwrap().into_string()?;
                    let (addr, map, scope) = stack.pop().unwrap().into_block()?;
                    let id = map.get(&*name).unwrap();
                    let ret_scope = mem::replace(&mut self.scope, scope);

                    self.call_stack.push((i + 1, ret_scope));
                    i = id * 2 + addr;
                    continue;
                }
                Index => {
                    let index = stack.pop().unwrap().into_number()?;
                    let list = stack.pop().unwrap().into_list()?;
                    stack.push(list.get(index as usize).unwrap().clone());

                    i += 1;
                    continue;
                }
            }
        }
        Ok(stack.pop().unwrap())
    }
}
