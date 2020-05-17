use crate::stack::Cmd;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Function(usize),
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

    fn into_function_addr(self) -> Result<usize> {
        match self {
            Value::Function(addr) => Ok(addr),
            _ => Err(anyhow!("{:?} is not function", self)),
        }
    }
}

type CallEnv = (usize, usize);

pub fn run(program: Vec<Cmd>) -> Result<String> {
    let mut i: usize = 0;
    let mut label_map: HashMap<usize, usize> = HashMap::new();
    let mut stack: Vec<Value> = Vec::new();
    let mut call_stack: Vec<CallEnv> = Vec::new();
    while program.len() > i {
        // dbg!(i, program[i].clone(), stack.clone(), call_stack.clone());
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
            Equal => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(Value::Bool(l == r));
            }
            NotEqual => {
                let r = stack.pop().unwrap();
                let l = stack.pop().unwrap();
                stack.push(Value::Bool(l != r));
            }
            NumberConst(n) => {
                stack.push(Value::Number(n));
            }
            StringConst(ref s) => {
                stack.push(Value::String(s.clone()));
            }
            Label(id) => {
                let cnt = stack.pop().unwrap().into_number()?;
                label_map.insert(id, cnt as usize);
            }
            LabelAddr(id) => {
                let cnt = label_map.get(&id).unwrap();
                stack.push(Value::Function(*cnt));
            }
            ProgramCounter => {
                stack.push(Value::Number(i as f64));
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
                let ret = stack.pop().unwrap();
                let (ret_addr, base_counter) = call_stack.pop().unwrap();

                stack.truncate(base_counter);
                stack.push(ret);
                i = ret_addr;
                continue;
            }
            Store => {
                call_stack.last_mut().unwrap().1 -= 1;
            }
            Load(i, depth) => {
                let (_, base_counter) = call_stack.get(call_stack.len() - 1 - depth).unwrap();
                let v = stack.get(base_counter + i).unwrap().clone();
                stack.push(v);
            }
            FunctionAddr => {
                let addr = stack.pop().unwrap().into_number()?;
                stack.push(Value::Function(addr as usize));
            }
            Call => {
                let addr = stack.pop().unwrap().into_function_addr()?;
                let ret_addr = i + 1;
                i = addr;
                call_stack.push((ret_addr, stack.len()));
                continue;
            }
        }

        i += 1;
    }
    Ok(format!("{:?}", stack))
}
