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
pub struct Value {
    primitive: Primitive,
    field: Rc<HashMap<String, usize>>,
    call_stack: CallStack
}

#[derive(Clone, Debug)]
pub enum Primitive {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Function(Function),
    Array(Rc<Vec<Function>>),
    Struct,
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

    pub fn string(s: Rc<String>) -> Value {
        Value {
            primitive: Primitive::String(s),
            field: Rc::new(HashMap::new()),
            call_stack: CallStack(None),
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

    pub fn into_struct(self) -> Result<(Rc<HashMap<String, usize>>, CallStack)> {
        match self.primitive {
            Primitive::Struct => Ok((self.field, self.call_stack)),
            _ => Err(anyhow!("{:?} is not struct", self.primitive)),
        }
    }

    pub fn array(v: Rc<Vec<Function>>) -> Value {
        Value {
            primitive: Primitive::Array(v),
            field: Rc::new(HashMap::new()),
            call_stack: CallStack(None),
        }
    }

    pub fn into_array(self) -> Result<Rc<Vec<Function>>> {
        match self.primitive {
            Primitive::Array(v) => Ok(v),
            _ => Err(anyhow!("{:?} is not array", self.primitive)),
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
        dbg!(program.clone());
        let mut i: usize = 0;
        let mut stack: Vec<Value> = Vec::new();
        while program.len() > i {
            use Cmd::*;
            dbg!(
                i,
                program[i].clone(),
                stack.clone(),
                self.call_stack.clone()
            );
            println!();
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
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::bool((l - r).abs() < f64::EPSILON));
                }
                NotEqual => {
                    let r = stack.pop().unwrap().into_number()?;
                    let l = stack.pop().unwrap().into_number()?;
                    stack.push(Value::bool((l - r).abs() > f64::EPSILON));
                }
                NumberConst(n) => {
                    stack.push(Value::number(n));
                }
                StringConst(ref s) => {
                    stack.push(Value::string(s.clone()));
                }
                ArrayConst(len) => {
                    let mut vec = Vec::new();
                    for _ in 0..len {
                        vec.push(stack.pop().unwrap().into_function()?);
                    }
                    vec.reverse();
                    stack.push(Value::array(Rc::new(vec)));
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
                    stack.push(Value::function(Function::Native(
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
                    let (map, call_stack) = stack.pop().unwrap().into_struct()?;
                    let id = map.get(&*name).unwrap();

                    let body = self.label_map.get(&id).unwrap();
                    stack.push(Value::function(Function::Native(body.clone(), call_stack)));
                }
                Index => {
                    let index = stack.pop().unwrap().into_number()?;
                    let array = stack.pop().unwrap().into_array()?;
                    stack.push(Value::function(array.get(index as usize).unwrap().clone()));
                }
            }

            i += 1;
        }
        Ok(stack.pop().unwrap())
    }
}
