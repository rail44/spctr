use crate::stack::Cmd;
use anyhow::{anyhow, Result};
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
    primitive: Primitive,
    field: Rc<HashMap<String, usize>>,
    call_stack: CallStack,
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

    pub fn string(v: Rc<String>) -> Value {
        let mut field = HashMap::new();
        field.insert("append".to_string(), 0_usize);
        let mut frame = Vec::new();

        let cloned = v.clone();
        frame.push(Rc::from(vec![Cmd::ForeignFunction(ForeignFunction(Rc::new(move |_, mut args| {
            let dst = args.pop().unwrap().into_string().unwrap();
            let v = format!("{}{}", v, dst);
            Value::string(Rc::new(v))
        })))]));

        let mut cs = CallStack(None);
        cs.push(frame);

        Value {
            primitive: Primitive::String(cloned),
            field: Rc::new(field),
            call_stack: cs
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

    pub fn array(v: Rc<Vec<Function>>) -> Value {
        let mut field = HashMap::new();
        field.insert("append".to_string(), 0_usize);
        let mut frame = Vec::new();

        let cloned = v.clone();
        frame.push(Rc::from(vec![Cmd::ForeignFunction(ForeignFunction(Rc::new(move |_, mut args| {
            let mut v = (*v).clone();
            let dst = args.pop().unwrap().into_array().unwrap();
            v.append(&mut (*dst).clone());
            Value::array(Rc::new(v))
        })))]));

        let mut cs = CallStack(None);
        cs.push(frame);

        Value {
            primitive: Primitive::Array(cloned),
            field: Rc::new(field),
            call_stack: cs
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

type StackFrame = Vec<Rc<[Cmd]>>;

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

    fn parent_nth(&self, n: usize) -> &StackFrame {
        let rc = self.0.as_ref().unwrap();
        if n == 0 {
            return &rc.0;
        }
        rc.1.parent_nth(n - 1)
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
        // dbg!(program.clone());
        let mut i: usize = 0;
        let mut stack: Vec<Value> = Vec::new();
        while program.len() > i {
            use Cmd::*;
            // dbg!(program[i].clone(), stack.clone(), self.call_stack.clone());
            // println!();
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
                Block(ref def_addrs, body_len) => {
                    let mut frame = Vec::new();
                    let mut body_base = i + 1;
                    for addr in def_addrs.iter() {
                        let body_range = body_base..body_base + addr;
                        body_base += addr;
                        frame.push(Rc::from(&program[body_range]));
                    }
                    let body_range = body_base..body_base + body_len;

                    self.call_stack.push(frame);
                    stack.push(self.run(&program[body_range])?);
                    self.call_stack.pop();

                    i = body_base + body_len;
                    continue;
                }
                Push(ref v) => {
                    stack.push(*v.clone());
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
                    stack.push(self.run(&v)?);
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

                            let mut defs = Vec::new();
                            for arg in args {
                                defs.push(Rc::from(vec![Cmd::Push(Box::new(arg))]));
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
                    let array = stack.pop().unwrap().into_array()?;
                    stack.push(Value::function(array.get(index as usize).unwrap().clone()));
                }
            }

            i += 1;
        }
        Ok(stack.pop().unwrap())
    }
}
