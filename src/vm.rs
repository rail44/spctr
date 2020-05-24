use crate::stack::Cmd;
use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub struct ForeignFunction(pub Rc<dyn Fn(&CallStack, Vec<Value>) -> Value>);

impl fmt::Debug for ForeignFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[native function]")
    }
}

#[derive(Clone, Debug)]
pub struct Value {
    pub primitive: Primitive,
    pub field: Rc<HashMap<String, usize>>,
    call_stack: CallStack,
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
            Primitive::Struct => {
                let mut vm = VM::new();
                vm.call_stack = self.call_stack.clone();
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
    Struct,
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
    Native(Rc<[Cmd]>, CallStack),
    Foreign(ForeignFunction),
}

impl Value {
    pub fn number(f: f64) -> Value {
        Value {
            primitive: Primitive::Number(f),
            field: Rc::new(HashMap::new()),
            call_stack: CallStack(None),
        }
    }

    pub fn null() -> Value {
        Value {
            primitive: Primitive::Null,
            field: Rc::new(HashMap::new()),
            call_stack: CallStack(None),
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
            call_stack: CallStack(None),
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
            call_stack: CallStack(None),
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
        let mut frame = Vec::new();

        let cloned = v.clone();
        frame.push(Rc::new(RefCell::new(Bind::Evalueated(Value::function(
            Function::Foreign(ForeignFunction(Rc::new(move |_, mut args| {
                let dst = args.pop().unwrap().into_string().unwrap();
                let v = format!("{}{}", v, dst);
                Value::string(Rc::new(v))
            }))),
        )))));

        let mut cs = CallStack(None);
        cs.push(frame);

        Value {
            primitive: Primitive::String(cloned),
            field: Rc::new(field),
            call_stack: cs,
        }
    }

    pub fn into_string(self) -> Result<Rc<String>> {
        match self.primitive {
            Primitive::String(s) => Ok(s),
            _ => Err(anyhow!("{:?} is not string", self.primitive)),
        }
    }

    pub fn struct_(field: Rc<HashMap<String, usize>>, call_stack: CallStack) -> Value {
        Value {
            primitive: Primitive::Struct,
            field,
            call_stack,
        }
    }

    pub fn list(v: Rc<Vec<Value>>) -> Value {
        let mut field = HashMap::new();
        field.insert("concat".to_string(), 0_usize);
        field.insert("to_iter".to_string(), 1_usize);
        let mut frame = Vec::new();

        let cloned = v.clone();
        frame.push(Rc::new(RefCell::new(Bind::Evalueated(Value::function(
            Function::Foreign(ForeignFunction(Rc::new(move |_, mut args| {
                let mut v = (*v).clone();
                let dst = args.pop().unwrap().into_list().unwrap();
                v.append(&mut (*dst).clone());
                Value::list(Rc::new(v))
            }))),
        )))));
        let v = cloned;

        let cloned = v.clone();
        frame.push(Rc::new(RefCell::new(Bind::Evalueated(Value::function(
            Function::Foreign(ForeignFunction(Rc::new(move |_, mut args| {
                let mut v = (*v).clone();
                let dst = args.pop().unwrap().into_list().unwrap();
                v.append(&mut (*dst).clone());
                Value::list(Rc::new(v))
            }))),
        )))));
        let v = cloned;

        let mut cs = CallStack(None);
        cs.push(frame);

        Value {
            primitive: Primitive::List(v),
            field: Rc::new(field),
            call_stack: cs,
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
    Cmd(Vec<Cmd>),
    Evalueated(Value),
}

#[derive(Clone, Debug)]
pub struct CallStack(Option<Rc<(StackFrame, CallStack)>>);

type StackFrame = Vec<Rc<RefCell<Bind>>>;

impl CallStack {
    fn push(&mut self, env: StackFrame) {
        self.0.replace(Rc::new((env, self.clone())));
    }

    fn pop(&mut self) -> StackFrame {
        let rc = self.0.take().unwrap();
        let (head, tail) = Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone());
        *self = tail;
        head
    }
}

pub fn run(program: &[Cmd]) -> Result<Value> {
    let mut vm = VM::new();
    vm.run(program)
}

struct VM {
    call_stack: CallStack,
}

impl VM {
    fn new() -> VM {
        let call_stack: CallStack = CallStack(None);
        VM { call_stack }
    }

    fn run(&mut self, program: &[Cmd]) -> Result<Value> {
        let mut i: usize = 0;
        let mut stack: Vec<Value> = Vec::new();
        while program.len() > i {
            use Cmd::*;
            match program[i] {
                Add => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l + r));
                }
                Sub => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l - r));
                }
                Mul => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l * r));
                }
                Div => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l / r));
                }
                Surplus => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::number(l % r));
                }
                Equal => {
                    let r = stack.pop().unwrap();
                    let l = stack.pop().unwrap();
                    stack.push(Value::bool(r == l));
                }
                Not => {
                    let b = stack.pop().unwrap().into_bool()?;
                    stack.push(Value::bool(!b));
                }
                Cmd::GreaterThan => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::bool(l > r));
                }
                Cmd::LessThan => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::bool(l < r));
                }
                NumberConst(n) => {
                    stack.push(Value::number(n));
                }
                StringConst(ref s) => {
                    stack.push(Value::string(s.clone()));
                }
                ConstructList(size) => {
                    let mut vec = Vec::new();
                    for _ in 0..size {
                        let v = stack.pop().unwrap();
                        vec.push(v);
                    }
                    vec.reverse();
                    stack.push(Value::list(Rc::new(vec)));
                }
                NullConst => {
                    stack.push(Value::null());
                }
                Block(ref def_addrs) => {
                    let mut frame = Vec::new();
                    let mut body_base = i + 1;
                    for addr in def_addrs.iter() {
                        let body_range = body_base..body_base + addr;
                        body_base += addr;
                        frame.push(Rc::new(RefCell::new(Bind::Cmd(
                            program[body_range].to_vec(),
                        ))));
                    }
                    i = body_base;

                    self.call_stack.push(frame);

                    continue;
                }
                Return => {
                    self.call_stack.pop();
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
                }
                Load(i, depth) => {
                    let ret = self.call_stack.clone();
                    let mut frame = self.call_stack.pop();
                    for _ in 0..depth {
                        frame = self.call_stack.pop();
                    }
                    self.call_stack.push(frame.clone());
                    let bind = frame.get(i).unwrap();
                    let inner = bind.try_borrow()?.clone();
                    let v = match inner {
                        Bind::Evalueated(v) => v,
                        Bind::Cmd(cmd) => {
                            let v = self.run(&cmd)?;
                            *bind.try_borrow_mut()? = Bind::Evalueated(v.clone());
                            v
                        }
                    };
                    stack.push(v);
                    self.call_stack = ret;
                }
                ConstructFunction(len) => {
                    let body_base = i + 1;
                    let body_range = body_base..body_base + len;
                    stack.push(Value::function(Function::Native(
                        Rc::from(&program[body_range]),
                        self.call_stack.clone(),
                    )));
                    i += len + 1;
                    continue;
                }
                ForeignFunction(ref func) => {
                    stack.push(Value::function(Function::Foreign(func.clone())));
                }
                StructAddr(ref map) => {
                    stack.push(Value::struct_(map.clone(), self.call_stack.clone()));
                }
                Call(arg_len) => {
                    let mut args = Vec::new();
                    for _ in 0..arg_len {
                        args.push(stack.pop().unwrap());
                    }

                    args.reverse();

                    match stack.pop().unwrap().into_function()? {
                        Function::Native(body, closure_call_stack) => {
                            let ret_frame = self.call_stack.clone();

                            self.call_stack = closure_call_stack;

                            let mut defs = Vec::new();
                            for arg in args {
                                defs.push(Rc::new(RefCell::new(Bind::Evalueated(arg))));
                            }
                            self.call_stack.push(defs);

                            stack.push(self.run(&body)?);

                            self.call_stack = ret_frame;
                        }
                        Function::Foreign(func) => {
                            stack.push(func.0(&self.call_stack, args));
                        }
                    }
                }
                Access => {
                    let name = stack.pop().unwrap().into_string()?;
                    let target = stack.pop().unwrap();
                    let map = target.field;
                    let call_stack = target.call_stack;
                    let id = map.get(&*name).unwrap();
                    let ret_frame = self.call_stack.clone();

                    self.call_stack = call_stack;

                    stack.push(self.run(&[Cmd::Load(*id, 0)])?);

                    self.call_stack = ret_frame;
                }
                Index => {
                    let index = stack.pop().unwrap().into_number()?;
                    let list = stack.pop().unwrap().into_list()?;
                    stack.push(list.get(index as usize).unwrap().clone());
                }
            }

            i += 1;
        }
        Ok(stack.pop().unwrap())
    }
}
