use crate::ast::{BinOp, Expr, Spanned, Statement, UnaryOp};
use crate::diag::Diagnostic;
use crate::lexer::Span;
use crate::parser;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    String(Rc<String>),
    Null,
    List(Rc<Vec<Value>>),
    Function(Rc<Function>),
    Block(Rc<Frame>),
}

pub enum Function {
    Closure {
        params: Vec<String>,
        body: Spanned<Expr>,
        env: Env,
    },
    Foreign {
        f: Box<dyn Fn(Vec<Value>, &Span) -> Result<Value, Diagnostic>>,
    },
}

#[derive(Clone)]
pub struct Env(Option<Rc<Frame>>);

pub struct Frame {
    bindings: RefCell<HashMap<String, Rc<RefCell<LazyState>>>>,
    parent: Env,
}

enum LazyState {
    Pending { expr: Spanned<Expr>, env: Env },
    Evaluating,
    Evaluated(Value),
}

impl Env {
    pub fn empty() -> Env {
        Env(None)
    }

    pub fn extend_evaluated(&self, bindings: HashMap<String, Value>) -> Env {
        let mut entries: HashMap<String, Rc<RefCell<LazyState>>> = HashMap::new();
        for (name, value) in bindings {
            entries.insert(name, Rc::new(RefCell::new(LazyState::Evaluated(value))));
        }
        Env(Some(Rc::new(Frame {
            bindings: RefCell::new(entries),
            parent: self.clone(),
        })))
    }

    fn lookup(&self, name: &str) -> Option<Rc<RefCell<LazyState>>> {
        let frame = self.0.as_ref()?;
        if let Some(v) = frame.bindings.borrow().get(name) {
            return Some(v.clone());
        }
        frame.parent.lookup(name)
    }
}

pub fn make_block(bindings: HashMap<String, Value>) -> Value {
    let mut entries: HashMap<String, Rc<RefCell<LazyState>>> = HashMap::new();
    for (name, value) in bindings {
        entries.insert(name, Rc::new(RefCell::new(LazyState::Evaluated(value))));
    }
    Value::Block(Rc::new(Frame {
        bindings: RefCell::new(entries),
        parent: Env::empty(),
    }))
}

pub fn make_foreign(
    f: impl Fn(Vec<Value>, &Span) -> Result<Value, Diagnostic> + 'static,
) -> Value {
    Value::Function(Rc::new(Function::Foreign { f: Box::new(f) }))
}

pub fn run(ast: &Statement) -> Result<Value, Diagnostic> {
    let env = build_global_env()?;
    interpret_statement(ast, &env)
}

fn build_global_env() -> Result<Env, Diagnostic> {
    let list_block = crate::stdlib::list::module();
    let string_block = crate::stdlib::string::module();

    let mut stdlib_bindings = HashMap::new();
    stdlib_bindings.insert("List".to_string(), list_block.clone());
    stdlib_bindings.insert("String".to_string(), string_block.clone());
    let stdlib_env = Env::empty().extend_evaluated(stdlib_bindings);

    let iter_ast = parser::parse(include_str!("stdlib/iterator.spc"))
        .expect("stdlib/iterator.spc parse failure");
    let iter_value = interpret_statement(&iter_ast, &stdlib_env)?;

    let mut bindings = HashMap::new();
    bindings.insert("List".to_string(), list_block);
    bindings.insert("String".to_string(), string_block);
    bindings.insert("Iterator".to_string(), iter_value);
    Ok(Env::empty().extend_evaluated(bindings))
}

fn interpret_statement(stmt: &Statement, env: &Env) -> Result<Value, Diagnostic> {
    let inner_env = make_lazy_frame(&stmt.definitions, env);
    interpret_expr(&stmt.body, &inner_env)
}

fn make_lazy_frame(defs: &[crate::ast::Bind], parent_env: &Env) -> Env {
    let frame = Rc::new(Frame {
        bindings: RefCell::new(HashMap::new()),
        parent: parent_env.clone(),
    });
    let inner_env = Env(Some(frame.clone()));

    let mut bindings = HashMap::new();
    for ((name, _), value_expr) in defs {
        let lazy = LazyState::Pending {
            expr: value_expr.clone(),
            env: inner_env.clone(),
        };
        bindings.insert(name.clone(), Rc::new(RefCell::new(lazy)));
    }
    *frame.bindings.borrow_mut() = bindings;

    inner_env
}

fn force(lazy: &Rc<RefCell<LazyState>>, span: &Span) -> Result<Value, Diagnostic> {
    {
        let borrowed = lazy.borrow();
        match &*borrowed {
            LazyState::Evaluated(v) => return Ok(v.clone()),
            LazyState::Evaluating => {
                return Err(Diagnostic::new(
                    span.clone(),
                    "cyclic binding",
                    "evaluation depends on itself",
                ));
            }
            LazyState::Pending { .. } => {}
        }
    }

    let state = std::mem::replace(&mut *lazy.borrow_mut(), LazyState::Evaluating);
    let (expr, env) = match state {
        LazyState::Pending { expr, env } => (expr, env),
        _ => unreachable!(),
    };

    match interpret_expr(&expr, &env) {
        Ok(v) => {
            *lazy.borrow_mut() = LazyState::Evaluated(v.clone());
            Ok(v)
        }
        Err(e) => {
            *lazy.borrow_mut() = LazyState::Pending { expr, env };
            Err(e)
        }
    }
}

fn interpret_expr(expr: &Spanned<Expr>, env: &Env) -> Result<Value, Diagnostic> {
    match &expr.0 {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::String(s) => Ok(Value::String(Rc::new(s.clone()))),
        Expr::Null => Ok(Value::Null),
        Expr::Variable(name) => {
            let lazy = env.lookup(name).ok_or_else(|| {
                Diagnostic::new(
                    expr.1.clone(),
                    format!("undefined variable: {}", name),
                    "not found in scope",
                )
            })?;
            force(&lazy, &expr.1)
        }
        Expr::List(items) => {
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(interpret_expr(item, env)?);
            }
            Ok(Value::List(Rc::new(values)))
        }
        Expr::Function(params, body) => {
            let param_names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
            Ok(Value::Function(Rc::new(Function::Closure {
                params: param_names,
                body: body.as_ref().clone(),
                env: env.clone(),
            })))
        }
        Expr::Block(defs) => {
            let frame = Rc::new(Frame {
                bindings: RefCell::new(HashMap::new()),
                parent: env.clone(),
            });
            let inner_env = Env(Some(frame.clone()));
            let mut bindings = HashMap::new();
            for ((name, _), value_expr) in defs {
                let lazy = LazyState::Pending {
                    expr: value_expr.clone(),
                    env: inner_env.clone(),
                };
                bindings.insert(name.clone(), Rc::new(RefCell::new(lazy)));
            }
            *frame.bindings.borrow_mut() = bindings;
            Ok(Value::Block(frame))
        }
        Expr::ImmediateBlock(stmt) => interpret_statement(stmt, env),
        Expr::If { cond, cons, alt } => {
            let cond_val = interpret_expr(cond, env)?;
            let b = match cond_val {
                Value::Bool(b) => b,
                v => {
                    return Err(Diagnostic::new(
                        cond.1.clone(),
                        "if condition must be a boolean",
                        format!("got {}", type_name(&v)),
                    ));
                }
            };
            if b {
                interpret_expr(cons, env)
            } else {
                interpret_expr(alt, env)
            }
        }
        Expr::Binary(op, l, r) => {
            let l_val = interpret_expr(l, env)?;
            let r_val = interpret_expr(r, env)?;
            apply_binary(*op, l_val, r_val, &expr.1)
        }
        Expr::Unary(op, e) => {
            let v = interpret_expr(e, env)?;
            apply_unary(*op, v, &expr.1)
        }
        Expr::Call(callee, args) => {
            let callee_val = interpret_expr(callee, env)?;
            let mut arg_vals = Vec::with_capacity(args.len());
            for a in args {
                arg_vals.push(interpret_expr(a, env)?);
            }
            call_function(&callee_val, arg_vals, &expr.1)
        }
        Expr::Access(obj, (name, _)) => {
            let obj_val = interpret_expr(obj, env)?;
            access_field(&obj_val, name, &expr.1)
        }
        Expr::Index(arr, idx) => {
            let arr_val = interpret_expr(arr, env)?;
            let idx_val = interpret_expr(idx, env)?;
            index_value(&arr_val, &idx_val, &expr.1)
        }
    }
}

fn apply_binary(op: BinOp, l: Value, r: Value, span: &Span) -> Result<Value, Diagnostic> {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            let ln = as_number(&l, span)?;
            let rn = as_number(&r, span)?;
            Ok(Value::Number(match op {
                BinOp::Add => ln + rn,
                BinOp::Sub => ln - rn,
                BinOp::Mul => ln * rn,
                BinOp::Div => ln / rn,
                BinOp::Mod => ln % rn,
                _ => unreachable!(),
            }))
        }
        BinOp::Eq => Ok(Value::Bool(values_equal(&l, &r))),
        BinOp::Ne => Ok(Value::Bool(!values_equal(&l, &r))),
        BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => {
            let ln = as_number(&l, span)?;
            let rn = as_number(&r, span)?;
            Ok(Value::Bool(match op {
                BinOp::Gt => ln > rn,
                BinOp::Lt => ln < rn,
                BinOp::Ge => ln >= rn,
                BinOp::Le => ln <= rn,
                _ => unreachable!(),
            }))
        }
    }
}

fn apply_unary(op: UnaryOp, v: Value, span: &Span) -> Result<Value, Diagnostic> {
    match op {
        UnaryOp::Neg => Ok(Value::Number(-as_number(&v, span)?)),
        UnaryOp::Not => Ok(Value::Bool(!as_bool(&v, span)?)),
    }
}

fn call_function(callee: &Value, args: Vec<Value>, span: &Span) -> Result<Value, Diagnostic> {
    let func = match callee {
        Value::Function(f) => f.clone(),
        _ => {
            return Err(Diagnostic::new(
                span.clone(),
                "not callable",
                format!("expected function, got {}", type_name(callee)),
            ));
        }
    };

    match &*func {
        Function::Closure { params, body, env } => {
            if args.len() != params.len() {
                return Err(Diagnostic::new(
                    span.clone(),
                    "arity mismatch",
                    format!("expected {} args, got {}", params.len(), args.len()),
                ));
            }
            let mut bindings = HashMap::new();
            for (name, value) in params.iter().zip(args) {
                bindings.insert(name.clone(), value);
            }
            let inner_env = env.extend_evaluated(bindings);
            interpret_expr(body, &inner_env)
        }
        Function::Foreign { f } => f(args, span),
    }
}

fn access_field(obj: &Value, name: &str, span: &Span) -> Result<Value, Diagnostic> {
    match obj {
        Value::Block(frame) => {
            let lazy = frame.bindings.borrow().get(name).cloned();
            match lazy {
                Some(l) => force(&l, span),
                None => Err(Diagnostic::new(
                    span.clone(),
                    format!("no field '{}'", name),
                    "not found in block",
                )),
            }
        }
        _ => Err(Diagnostic::new(
            span.clone(),
            "field access on non-block",
            format!("{} has no fields", type_name(obj)),
        )),
    }
}

fn index_value(arr: &Value, idx: &Value, span: &Span) -> Result<Value, Diagnostic> {
    match arr {
        Value::List(list) => {
            let i = as_number(idx, span)? as usize;
            list.get(i).cloned().ok_or_else(|| {
                Diagnostic::new(
                    span.clone(),
                    format!("index {} out of bounds", i),
                    "out of bounds",
                )
            })
        }
        Value::Block(frame) => {
            let name = match idx {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(Diagnostic::new(
                        span.clone(),
                        "block index must be a string",
                        format!("got {}", type_name(idx)),
                    ));
                }
            };
            let lazy = frame.bindings.borrow().get(name.as_str()).cloned();
            match lazy {
                Some(l) => force(&l, span),
                None => Err(Diagnostic::new(
                    span.clone(),
                    format!("no field '{}'", name),
                    "not found",
                )),
            }
        }
        _ => Err(Diagnostic::new(
            span.clone(),
            "indexing on non-list/block",
            format!("cannot index {}", type_name(arr)),
        )),
    }
}

fn as_number(v: &Value, span: &Span) -> Result<f64, Diagnostic> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err(Diagnostic::new(
            span.clone(),
            "type mismatch",
            format!("expected number, got {}", type_name(v)),
        )),
    }
}

fn as_bool(v: &Value, span: &Span) -> Result<bool, Diagnostic> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(Diagnostic::new(
            span.clone(),
            "type mismatch",
            format!("expected bool, got {}", type_name(v)),
        )),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x - y).abs() < f64::EPSILON,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::String(_) => "string",
        Value::Null => "null",
        Value::List(_) => "list",
        Value::Function(_) => "function",
        Value::Block(_) => "block",
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Function(_) => write!(f, "[function]"),
            Value::Block(frame) => {
                let names: Vec<String> = frame.bindings.borrow().keys().cloned().collect();
                write!(f, "{{ {} }}", names.join(", "))
            }
        }
    }
}
