use crate::stack::Cmd;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug)]
enum Value {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Array(Rc<Vec<Value>>),
    Function(usize, CallStack),
    Struct(Rc<HashMap<String, usize>>),
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

    fn into_function(self) -> Result<(usize, CallStack)> {
        match self {
            Value::Function(addr, call_stack) => Ok((addr, call_stack)),
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
}

#[derive(Clone, Debug)]
struct CallStack(Option<Rc<(StackFrame, CallStack)>>);

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
                    vec.push(stack.pop().unwrap());
                }
                vec.reverse();
                stack.push(Value::Array(Rc::new(vec)));
            }
            Label(id, addr) => {
                label_map.insert(id, i + addr);
            }
            LabelAddr(id) => {
                let cnt = label_map.get(&id).unwrap();
                stack.push(Value::Function(*cnt, call_stack.clone()));
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
                stack.push(Value::Function(addr + i, call_stack.clone()));
            }
            StructAddr(ref map) => {
                stack.push(Value::Struct(map.clone()));
            }
            Call(arg_len) => {
                let mut args = Vec::new();
                for _ in 0..arg_len {
                    args.push(stack.pop().unwrap());
                }

                let (addr, closure_call_stack) = stack.pop().unwrap().into_function()?;
                let ret_addr = i + 1;
                let ret_frame = call_stack;

                call_stack = closure_call_stack;
                call_stack.push((ret_addr, ret_frame, args));

                i = addr;
                continue;
            }
            Access => {
                let name = stack.pop().unwrap().into_string()?;
                let map = stack.pop().unwrap().into_struct()?;
                let id = map.get(&*name).unwrap();

                let cnt = label_map.get(&id).unwrap();
                stack.push(Value::Function(*cnt, call_stack.clone()));
            }
        }

        i += 1;
    }
    Ok(format!("{:?}", stack))
}
