use crate::translator::Env;
use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::mem;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum Cmd {
    Add,
    Sub,
    Div,
    Mul,
    Surplus,
    Equal,
    GreaterThan,
    LessThan,
    Not,
    Load(usize, usize),
    Store(usize),
    Block(Vec<usize>),
    NumberConst(f64),
    StringConst(Rc<String>),
    NullConst,
    ConstructList(usize),
    ConstructFunction(usize, usize),
    ConstructBlock(usize, Rc<HashMap<String, usize>>),
    ConstructForeignFunction(ForeignFunction, Env),
    JumpRel(usize),
    JumpRelUnless(usize),
    Call(usize),
    Index,
    Access,
    ExitScope,
    Return,
}

#[derive(Clone)]
pub struct ForeignFunction(pub Rc<dyn Fn(&Scope, Vec<Value>) -> Value>);

impl fmt::Debug for ForeignFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[foreign function]")
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(ref s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Function(_) => write!(f, "[function]"),
            Value::List(ref v) => {
                let fmt_values: Vec<_> = v.iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", fmt_values.join(", "))
            }
            Value::Null => write!(f, "null"),
            Value::Block((_, field, _)) => {
                write!(f, "{:?}", field)
                // let mut vm = VM::new();
                // vm.scope = self.scope.clone();
                // let fmt_entries: Vec<_> = self
                //     .field
                //     .iter()
                //     .map(|(k, v)| {
                //         let v = vm.run(&[Cmd::Load(*v, 0)]).unwrap();
                //         format!("{}: {}", k, v)
                //     })
                //     .collect();
                // write!(f, "{{{}}}", fmt_entries.join(", "))
            }
        }
    }
}

type Block = (usize, Rc<HashMap<String, usize>>, Scope);

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Function(Function),
    List(Rc<Vec<Value>>),
    Null,
    Block(Block),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Number(a), Number(b)) => (a - b).abs() < f64::EPSILON,
            (Null, Null) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub enum Function {
    Native(usize, usize, Scope),
    Foreign(ForeignFunction, Scope, Env),
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[function]")
    }
}

impl Value {
    pub fn number(f: f64) -> Value {
        Value::Number(f)
    }

    pub fn null() -> Value {
        Value::Null
    }

    pub fn into_number(self) -> Result<f64> {
        match self {
            Value::Number(n) => Ok(n),
            _ => Err(anyhow!("not number")),
        }
    }

    pub fn bool(b: bool) -> Value {
        Value::Bool(b)
    }

    pub fn into_bool(self) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(b),
            _ => Err(anyhow!("not bool")),
        }
    }

    pub fn function(f: Function) -> Value {
        Value::Function(f)
    }

    pub fn into_function(self) -> Result<Function> {
        match self {
            Value::Function(func) => Ok(func),
            _ => Err(anyhow!("{:?} is not function", self)),
        }
    }

    pub fn string(v: Rc<String>) -> Value {
        Value::String(v)
    }

    pub fn into_string(self) -> Result<Rc<String>> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(anyhow!("{:?} is not string", self)),
        }
    }

    pub fn block(i: usize, field: Rc<HashMap<String, usize>>, scope: Scope) -> Value {
        Value::Block((i, field, scope))
    }

    pub fn into_block(self) -> Result<Block> {
        match self {
            Value::Block(b) => Ok(b),
            _ => Err(anyhow!("{:?} is not block", self)),
        }
    }

    pub fn list(v: Rc<Vec<Value>>) -> Value {
        Value::List(v)
    }

    pub fn into_list(self) -> Result<Rc<Vec<Value>>> {
        match self {
            Value::List(v) => Ok(v),
            _ => Err(anyhow!("{:?} is not list", self)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Bind {
    Cmd(usize),
    Evalueated(Value),
}

#[derive(Clone, Debug, PartialEq)]
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
    let vm = VM::new(program);
    vm.run()
}

struct VM<'a> {
    scope: Scope,
    call_stack: Vec<(Option<usize>, usize, Scope)>,
    stack: Vec<Value>,
    i: usize,
    program: &'a [Cmd],
}

impl<'a> VM<'a> {
    fn new(program: &'a [Cmd]) -> VM {
        let scope: Scope = Scope(None);
        VM {
            scope,
            call_stack: Vec::new(),
            stack: Vec::new(),
            i: 0,
            program,
        }
    }

    fn run(mut self) -> Result<Value> {
        let len = self.program.len();
        while len > self.i {
            use Cmd::*;
            match self.program[self.i] {
                Add => self.add()?,
                Sub => self.sub()?,
                Mul => self.mul()?,
                Div => self.div()?,
                Surplus => self.surplus()?,
                Equal => self.equal()?,
                Not => self.not()?,
                GreaterThan => self.greater_than()?,
                LessThan => self.less_than()?,
                NumberConst(n) => self.number_const(n)?,
                StringConst(ref s) => self.string_const(s.clone())?,
                ConstructList(size) => self.list(size)?,
                NullConst => self.null()?,
                Block(ref def_addrs) => self.block(def_addrs)?,
                Return => self.return_()?,
                ExitScope => self.exit_scope()?,
                JumpRel(n) => self.jump_rel(n)?,
                JumpRelUnless(n) => self.jump_rel_unless(n)?,
                Load(i, depth) => self.load(i, depth)?,
                Store(i) => self.store(i)?,
                ConstructFunction(id, len) => self.function(id, len)?,
                ConstructForeignFunction(ref func, ref map) => {
                    self.foreign_function(func.clone(), map.clone())?
                }
                ConstructBlock(len, ref map) => self.construct_block(len, map.clone())?,
                Call(arg_len) => self.call(arg_len)?,
                Access => self.access()?,
                Index => self.index()?,
            };
        }
        Ok(self.stack.pop().unwrap())
    }

    fn add(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::number(l + r));
        self.i += 1;
        Ok(())
    }

    fn sub(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::number(l - r));
        self.i += 1;
        Ok(())
    }

    fn mul(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::number(l * r));
        self.i += 1;
        Ok(())
    }

    fn div(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::number(l / r));
        self.i += 1;
        Ok(())
    }

    fn surplus(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::number(l % r));
        self.i += 1;
        Ok(())
    }

    fn equal(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap();
        let l = self.stack.pop().unwrap();
        self.stack.push(Value::bool(r == l));
        self.i += 1;
        Ok(())
    }

    fn not(&mut self) -> Result<()> {
        let b = self.stack.pop().unwrap().into_bool()?;
        self.stack.push(Value::bool(!b));
        self.i += 1;
        Ok(())
    }

    fn greater_than(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::bool(l > r));
        self.i += 1;
        Ok(())
    }

    fn less_than(&mut self) -> Result<()> {
        let r = self.stack.pop().unwrap().into_number()?;
        let l = self.stack.pop().unwrap().into_number()?;
        self.stack.push(Value::bool(l < r));
        self.i += 1;
        Ok(())
    }

    fn number_const(&mut self, n: f64) -> Result<()> {
        self.stack.push(Value::number(n));
        self.i += 1;
        Ok(())
    }

    fn string_const(&mut self, s: Rc<String>) -> Result<()> {
        self.stack.push(Value::string(s));
        self.i += 1;
        Ok(())
    }

    fn list(&mut self, size: usize) -> Result<()> {
        let mut vec = Vec::new();
        for _ in 0..size {
            let v = self.stack.pop().unwrap();
            vec.push(v);
        }
        vec.reverse();
        self.stack.push(Value::list(Rc::new(vec)));
        self.i += 1;
        Ok(())
    }

    fn null(&mut self) -> Result<()> {
        self.stack.push(Value::null());
        self.i += 1;
        Ok(())
    }

    fn block(&mut self, def_addrs: &[usize]) -> Result<()> {
        let mut binds = Vec::new();
        let mut body_base = self.i + 1;
        for addr in def_addrs.iter() {
            binds.push(Rc::new(RefCell::new(Bind::Cmd(body_base))));
            body_base += addr;
        }
        self.i = body_base;

        self.scope.push(binds);

        Ok(())
    }

    fn exit_scope(&mut self) -> Result<()> {
        self.scope.pop();
        self.i += 1;
        Ok(())
    }

    fn jump_rel(&mut self, n: usize) -> Result<()> {
        self.i += n;
        Ok(())
    }

    fn jump_rel_unless(&mut self, n: usize) -> Result<()> {
        let cond = self.stack.pop().unwrap().into_bool()?;
        if !cond {
            self.i += n;
            return Ok(());
        }
        self.i += 1;
        Ok(())
    }

    fn load(&mut self, n: usize, depth: usize) -> Result<()> {
        let scope = self.scope.nth_parent(depth);
        let binds: &Binds = scope.0.as_ref().unwrap().0.as_ref();

        let bind = binds.get(n).unwrap();
        let inner = bind.borrow().clone();
        match inner {
            Bind::Evalueated(v) => {
                self.stack.push(v);
                self.i += 1;
                Ok(())
            }
            Bind::Cmd(addr) => {
                let ret_i = self.i + 1;
                let scope = scope.clone();
                let ret_scope = mem::replace(&mut self.scope, scope);

                self.call_stack.push((None, ret_i, ret_scope));
                self.i = addr;
                Ok(())
            }
        }
    }

    fn return_(&mut self) -> Result<()> {
        let (_, ret_i, ret_scope) = self.call_stack.pop().unwrap();
        self.i = ret_i;
        self.scope = ret_scope;
        Ok(())
    }

    fn store(&mut self, n: usize) -> Result<()> {
        let v = self.stack.pop().unwrap();
        let binds: &Binds = self.scope.0.as_ref().unwrap().0.as_ref();

        let bind = binds.get(n).unwrap();
        *bind.borrow_mut() = Bind::Evalueated(v.clone());
        self.stack.push(v);

        self.i += 1;
        Ok(())
    }

    fn function(&mut self, id: usize, len: usize) -> Result<()> {
        let body_base = self.i + 1;
        self.stack.push(Value::function(Function::Native(
            id,
            body_base,
            self.scope.clone(),
        )));
        self.i = body_base + len;
        Ok(())
    }

    fn foreign_function(
        &mut self,
        func: ForeignFunction,
        env: Env,
    ) -> Result<()> {
        self.stack.push(Value::function(Function::Foreign(
            func,
            self.scope.clone(),
            env,
        )));
        self.i += 1;
        Ok(())
    }

    fn construct_block(&mut self, len: usize, map: Rc<HashMap<String, usize>>) -> Result<()> {
        self.stack
            .push(Value::block(self.i + 1, map, self.scope.clone()));
        self.i += 1 + len;
        Ok(())
    }

    fn call(&mut self, arg_len: usize) -> Result<()> {
        let len = self.stack.len() - arg_len;
        let mut args = self.stack.split_off(len);

        match self.stack.pop().unwrap().into_function()? {
            Function::Native(id, addr, closure_scope) => {
                let mut defs = Vec::new();
                for arg in args {
                    defs.push(Rc::new(RefCell::new(Bind::Evalueated(arg))));
                }

                let mut stacked_scope = 0;
                let next_cmd = self.program[(self.i + 1)..].iter().find(|cmd| match cmd {
                    Cmd::ExitScope => {
                        stacked_scope += 1;
                        false
                    }
                    _ => true,
                });

                if let Some(Cmd::Return) = next_cmd {
                    let current_func_stack = self.call_stack.iter().rev().find(|cs| cs.0.is_some());

                    if let Some(cs) = current_func_stack {
                        if id == cs.0.unwrap() && cs.2 == closure_scope {
                            for _ in 0..stacked_scope {
                                self.scope.pop();
                            }
                            self.scope.push(defs);
                            self.i = addr;
                            return Ok(());
                        }
                    }
                }

                let ret_scope = mem::replace(&mut self.scope, closure_scope);
                self.scope.push(defs);

                self.call_stack.push((Some(id), self.i + 1, ret_scope));
                self.i = addr;
                Ok(())
            }
            Function::Foreign(func, scope, _map) => {
                args.reverse();
                self.stack.push(func.0(&scope, args));
                self.i += 1;
                Ok(())
            }
        }
    }

    fn access(&mut self) -> Result<()> {
        let name = self.stack.pop().unwrap().into_string()?;
        let (addr, map, scope) = self.stack.pop().unwrap().into_block()?;
        let id = map.get(&*name).unwrap();
        let ret_scope = mem::replace(&mut self.scope, scope);

        self.call_stack.push((None, self.i + 1, ret_scope));
        self.i = id * 2 + addr;
        Ok(())
    }

    fn index(&mut self) -> Result<()> {
        let index = self.stack.pop().unwrap().into_number()?;
        let list = self.stack.pop().unwrap().into_list()?;
        self.stack.push(list.get(index as usize).unwrap().clone());

        self.i += 1;
        Ok(())
    }
}
