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
    Array(Rc<Vec<usize>>),
    Struct(Rc<HashMap<String, usize>>),
    Function(Function),
}

#[derive(Clone, Debug)]
pub enum Function {
    Native(usize, CallStack),
    Foreign(ForeignFunction),
}

impl Value {
    fn into_number(self) -> Result<f64> {
        match self {
            Value::Number(n) => Ok(n),
            _ => Err(anyhow!("not number")),
        }
    }

    fn into_bool(self) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(b),
            _ => Err(anyhow!("not bool")),
        }
    }

    fn into_function(self) -> Result<Function> {
        match self {
            Value::Function(func) => Ok(func),
            _ => Err(anyhow!("{:?} is not function", self)),
        }
    }

    fn into_string(self) -> Result<Rc<String>> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(anyhow!("{:?} is not string", self)),
        }
    }

    fn into_struct(self) -> Result<Rc<HashMap<String, usize>>> {
        match self {
            Value::Struct(map) => Ok(map),
            _ => Err(anyhow!("{:?} is not struct", self)),
        }
    }

    fn into_array(self) -> Result<Rc<Vec<usize>>> {
        match self {
            Value::Array(v) => Ok(v),
            _ => Err(anyhow!("{:?} is not array", self)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CallStack(Option<Rc<(StackFrame, CallStack)>>);

type StackFrame = (usize, CallStack, Vec<Value>);

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

pub fn run(program: Vec<Cmd>) -> Result<String> {
    let mut i: usize = 0;
    let mut label_map: HashMap<usize, usize> = HashMap::new();
    let mut stack: Vec<Value> = Vec::new();
    let mut call_stack: CallStack = CallStack(None);
    while program.len() > i {
        use Cmd::*;
        dbg!(i, program[i].clone(), stack.clone(), call_stack.clone());
        println!();
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
            ArrayConst(ref addrs) => {
                let abs_addrs: Vec<_> = addrs.iter().map(|addr| i + addr).collect();
                stack.push(Value::Array(Rc::new(abs_addrs)));
            }
            Label(id, addr) => {
                label_map.insert(id, i + addr);
            }
            LabelAddr(id) => {
                let cnt = label_map.get(&id).unwrap();
                stack.push(Value::Function(Function::Native(*cnt, call_stack.clone())));
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
            Return => {
                let (ret_addr, ret_frame, _) = call_stack.pop();
                call_stack = ret_frame;
                i = ret_addr;
                continue;
            }
            Load(i, depth) => {
                let (_, _, args) = call_stack.parent_nth(depth);
                let v = args.get(i).unwrap().clone();
                stack.push(v);
            }
            FunctionAddr(addr) => {
                stack.push(Value::Function(Function::Native(
                    addr + i,
                    call_stack.clone(),
                )));
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
                    Function::Native(addr, closure_call_stack) => {
                        let ret_addr = i + 1;
                        let ret_frame = call_stack;

                        call_stack = closure_call_stack;
                        call_stack.push((ret_addr, ret_frame, args));

                        i = addr;
                        continue;
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

                let cnt = label_map.get(&id).unwrap();
                stack.push(Value::Function(Function::Native(*cnt, call_stack.clone())));
            }
            Index => {
                let index = stack.pop().unwrap().into_number()?;
                let array = stack.pop().unwrap().into_array()?;
                let addr = array.get(index as usize).unwrap();

                stack.push(Value::Function(Function::Native(*addr, call_stack.clone())));
            }
        }

        i += 1;
    }
    Ok(format!("{:?}", stack))
}
