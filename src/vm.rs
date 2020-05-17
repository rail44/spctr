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
            _ => Err(anyhow!("not bool")),
        }
    }
}

pub fn run(program: Vec<Cmd>) -> Result<String> {
    let mut i: usize = 0;
    let mut label_map: HashMap<usize, usize> = HashMap::new();
    let mut stack: Vec<Value> = Vec::new();
    let mut args: Vec<Vec<Value>> = vec!(Vec::new());
    while program.len() > i {
        use Cmd::*;
        dbg!(stack.clone());
        dbg!(program[i].clone());
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
            LabelCounter(id) => {
                let cnt = stack.pop().unwrap().into_number()?;
                label_map.insert(id, cnt as usize);
            }
            JumpToLabel(id) => {
                let cnt = label_map.get(&id).unwrap();
                i = *cnt as usize;
                continue;
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
                let addr = stack.pop().unwrap();
                stack.push(ret);
                i = addr.into_number()? as usize;
                continue;
            },
            FunctionReturn => {
                let ret = stack.pop().unwrap();
                let addr = stack.pop().unwrap();
                stack.push(ret);
                args.pop();
                i = addr.into_number()? as usize;
                continue;
            },
            Load(i) => {
                stack.push(args.last_mut().unwrap().get(i).unwrap().clone());
            }
            Store => {
                let value = stack.pop().unwrap();
                args.last_mut().unwrap().push(value);
            }
            FunctionAddr => {
                let addr = stack.pop().unwrap().into_number()?;
                stack.push(Value::Function(addr as usize));
            }
            Call => {
                let addr = stack.pop().unwrap().into_function_addr()?;
                i = addr;
                args.push(Vec::new());
                continue;
            }
        }

        i += 1;
    }
    Ok(format!("{:?}", stack.pop().unwrap()))
}
