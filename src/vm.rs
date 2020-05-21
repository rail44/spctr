use crate::stack::Cmd;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub struct ForeignFunction(pub Rc<dyn Fn(Vec<Value>) -> Value>);

impl fmt::Debug for ForeignFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[native function]")
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Array(Rc<Vec<Function>>),
    Struct(Rc<HashMap<String, usize>>),
    Function(Function),
}

#[derive(Clone, Debug)]
pub enum Function {
    Native(Rc<[Cmd]>, CallStack),
    Foreign(ForeignFunction),
}

impl Value {
    pub fn into_number(self) -> Result<f64> {
        match self {
            Value::Number(n) => Ok(n),
            _ => Err(anyhow!("not number")),
        }
    }

    pub fn into_bool(self) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(b),
            _ => Err(anyhow!("not bool")),
        }
    }

    pub fn into_function(self) -> Result<Function> {
        match self {
            Value::Function(func) => Ok(func),
            _ => Err(anyhow!("{:?} is not function", self)),
        }
    }

    pub fn into_string(self) -> Result<Rc<String>> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(anyhow!("{:?} is not string", self)),
        }
    }

    pub fn into_struct(self) -> Result<Rc<HashMap<String, usize>>> {
        match self {
            Value::Struct(map) => Ok(map),
            _ => Err(anyhow!("{:?} is not struct", self)),
        }
    }

    pub fn into_array(self) -> Result<Rc<Vec<Function>>> {
        match self {
            Value::Array(v) => Ok(v),
            _ => Err(anyhow!("{:?} is not array", self)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CallStack(Option<Rc<(StackFrame, CallStack)>>);

type StackFrame = Vec<Value>;

impl CallStack {
    fn push(&mut self, env: StackFrame) {
        let this = CallStack(self.0.take());
        self.0 = Some(Rc::new((env, this)));
    }

    fn pop(&mut self) -> StackFrame {
        let this = CallStack(self.0.take());
        let rc = this.0.unwrap();
        let (head, tail) = Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone());
        *self = tail;
        head
    }

    fn parent_nth(&self, n: usize) -> &StackFrame {
        let rc = self.0.as_ref().unwrap();
        if n == 0 {
            return &rc.0;
        }
        rc.1.parent_nth(n - 1)
    }
}

pub fn run(program: Vec<Cmd>) -> Result<Value> {
    let mut vm = VM::new();
    vm.run(program)
}

struct VM {
    label_map: HashMap<usize, Rc<[Cmd]>>,
    call_stack: CallStack,
}

impl VM {
    fn new() -> VM {
        let label_map: HashMap<usize, Rc<[Cmd]>> = HashMap::new();
        let call_stack: CallStack = CallStack(None);
        VM {
            label_map,
            call_stack,
        }
    }

    fn run(&mut self, program: Vec<Cmd>) -> Result<Value> {
        let mut i: usize = 0;
        let mut stack: Vec<Value> = Vec::new();
        while program.len() > i {
            use Cmd::*;
            match program[i] {
                Add => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Number(l + r));
                }
                Sub => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Number(l - r));
                }
                Mul => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Number(l * r));
                }
                Div => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Number(l / r));
                }
                Surplus => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Number(l % r));
                }
                Equal => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Bool(l == r));
                }
                NotEqual => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::Bool(l != r));
                }
                NumberConst(n) => {
                    stack.push(Value::Number(n));
                }
                StringConst(ref s) => {
                    stack.push(Value::String(s.clone()));
                }
                ArrayConst(len) => {
                    let mut vec = Vec::new();
                    for _ in 0..len {
                        vec.push(stack.pop().unwrap().into_function()?);
                    }
                    vec.reverse();
                    stack.push(Value::Array(Rc::new(vec)));
                }
                Label(id, len) => {
                    let body_base = i + 1;
                    let body_range = body_base..body_base + len;
                    self.label_map.insert(id, Rc::from(&program[body_range]));
                    i += len + 1;
                    continue;
                }
                LabelAddr(id) => {
                    let body = self.label_map.get(&id).unwrap();
                    stack.push(Value::Function(Function::Native(
                        body.clone(),
                        self.call_stack.clone(),
                    )));
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
                    let args = self.call_stack.parent_nth(depth);
                    let v = args.get(i).unwrap().clone();
                    stack.push(v);
                }
                ConstructFunction(len) => {
                    let body_base = i + 1;
                    let body_range = body_base..body_base + len;
                    stack.push(Value::Function(Function::Native(
                        Rc::from(&program[body_range]),
                        self.call_stack.clone(),
                    )));
                    i += len + 1;
                    continue;
                }
                ForeignFunction(ref func) => {
                    stack.push(Value::Function(Function::Foreign(func.clone())));
                }
                StructAddr(ref map) => {
                    stack.push(Value::Struct(map.clone()));
                }
                Call(arg_len) => {
                    let mut args = Vec::new();
                    for _ in 0..arg_len {
                        args.push(stack.pop().unwrap());
                    }

                    match stack.pop().unwrap().into_function()? {
                        Function::Native(body, closure_call_stack) => {
                            let ret_frame = self.call_stack.clone();

                            self.call_stack = closure_call_stack;
                            self.call_stack.push(args);

                            stack.push(self.run(body.to_vec())?);

                            self.call_stack.pop();
                            self.call_stack = ret_frame;
                        }
                        Function::Foreign(func) => {
                            stack.push(func.0(args));
                        }
                    }
                }
                Access => {
                    let name = stack.pop().unwrap().into_string()?;
                    let map = stack.pop().unwrap().into_struct()?;
                    let id = map.get(&*name).unwrap();

                    let cnt = self.label_map.get(&id).unwrap();
                    stack.push(Value::Function(Function::Native(
                        cnt.clone(),
                        self.call_stack.clone(),
                    )));
                }
                Index => {
                    let index = stack.pop().unwrap().into_number()?;
                    let array = stack.pop().unwrap().into_array()?;
                    stack.push(Value::Function(array.get(index as usize).unwrap().clone()));
                }
            }

            i += 1;
        }
        Ok(stack.pop().unwrap())
    }
}
