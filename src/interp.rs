use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::Span;
use crate::symbol::{display, intern, Symbol};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

pub type EvalResult = Result<Value, Diagnostic>;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    String(Rc<String>),
    Bool(bool),
    Null,
    List(Rc<Vec<Value>>),
    Function(Function),
    Block(Rc<Frame>),
}

#[derive(Clone)]
pub enum Function {
    Native {
        params: Vec<Symbol>,
        body: Rc<Spanned<Expr>>,
        env: Env,
    },
    Foreign(Rc<dyn Fn(Vec<Value>, &Span) -> EvalResult>),
}

#[derive(Clone, Default)]
pub struct Env(Option<Rc<Frame>>);

pub struct Frame {
    pub binds: Vec<Rc<RefCell<BindState>>>,
    pub names: Option<HashMap<Symbol, u32>>,
    pub parent: Env,
}

pub enum BindState {
    Lazy(Rc<Spanned<Expr>>),
    InProgress,
    Done(Value),
}

impl Env {
    pub fn empty() -> Self {
        Env(None)
    }

    fn parent_at(&self, depth: u32) -> Env {
        let mut cur = self.clone();
        for _ in 0..depth {
            let next = cur
                .0
                .as_ref()
                .expect("env chain shorter than resolved depth")
                .parent
                .clone();
            cur = next;
        }
        cur
    }
}

pub const ROOT_NAMES: [&str; 3] = ["Iterator", "List", "String"];

pub fn run(ast: &Statement) -> EvalResult {
    let env = build_root_env();
    interpret_statement(ast, &env)
}

fn build_root_env() -> Env {
    let iter_stmt = crate::parser::parse(include_str!("stdlib/iterator.spc"))
        .expect("stdlib/iterator.spc must parse");
    crate::resolver::resolve(&iter_stmt, &ROOT_NAMES).expect("stdlib/iterator.spc must resolve");
    let iter_expr = Rc::new((Expr::ImmediateBlock(Box::new(iter_stmt)), 0..0));

    let mut binds: Vec<Rc<RefCell<BindState>>> = Vec::with_capacity(ROOT_NAMES.len());

    binds.push(Rc::new(RefCell::new(BindState::Lazy(iter_expr))));
    binds.push(Rc::new(RefCell::new(BindState::Done(
        crate::stdlib::list::module(),
    ))));
    binds.push(Rc::new(RefCell::new(BindState::Done(
        crate::stdlib::string::module(),
    ))));

    Env(Some(Rc::new(Frame {
        binds,
        names: None,
        parent: Env::empty(),
    })))
}

fn force(env: &Env, bind: &Rc<RefCell<BindState>>, span: &Span) -> EvalResult {
    {
        let state = bind.borrow();
        match &*state {
            BindState::Done(v) => return Ok(v.clone()),
            BindState::InProgress => {
                return Err(Diagnostic::new(
                    span.clone(),
                    "cyclic binding",
                    "this binding refers to itself during evaluation",
                ))
            }
            BindState::Lazy(_) => {}
        }
    }

    let expr = match std::mem::replace(&mut *bind.borrow_mut(), BindState::InProgress) {
        BindState::Lazy(e) => e,
        _ => unreachable!(),
    };

    match interpret(&expr, env) {
        Ok(v) => {
            *bind.borrow_mut() = BindState::Done(v.clone());
            Ok(v)
        }
        Err(err) => {
            *bind.borrow_mut() = BindState::Lazy(expr);
            Err(err)
        }
    }
}

pub fn interpret_statement(stmt: &Statement, env: &Env) -> EvalResult {
    let frame = make_frame(&stmt.definitions, env, false);
    let new_env = Env(Some(Rc::new(frame)));
    interpret(&stmt.body, &new_env)
}

fn make_frame(defs: &[Bind], parent: &Env, with_names: bool) -> Frame {
    let mut binds = Vec::with_capacity(defs.len());
    let mut names = if with_names {
        Some(HashMap::with_capacity(defs.len()))
    } else {
        None
    };
    for (i, ((name, _), body)) in defs.iter().enumerate() {
        if let Some(n) = names.as_mut() {
            n.insert(*name, i as u32);
        }
        binds.push(Rc::new(RefCell::new(BindState::Lazy(Rc::new(body.clone())))));
    }
    Frame {
        binds,
        names,
        parent: parent.clone(),
    }
}

pub fn interpret(expr: &Spanned<Expr>, env: &Env) -> EvalResult {
    let (e, span) = (&expr.0, &expr.1);
    match e {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::String(s) => Ok(Value::String(s.clone())),
        Expr::Null => Ok(Value::Null),
        Expr::Variable(var) => {
            let bref = var.resolved.get().ok_or_else(|| {
                Diagnostic::new(
                    span.clone(),
                    format!("unresolved variable: {}", display(var.name)),
                    "resolver did not run",
                )
            })?;
            let env_at_def = env.parent_at(bref.depth);
            let bind = env_at_def
                .0
                .as_ref()
                .expect("frame missing")
                .binds
                .get(bref.slot as usize)
                .expect("slot out of range")
                .clone();
            force(&env_at_def, &bind, span)
        }
        Expr::List(items) => {
            let mut vs = Vec::with_capacity(items.len());
            for item in items {
                vs.push(interpret(item, env)?);
            }
            Ok(Value::List(Rc::new(vs)))
        }
        Expr::Function(params, body) => {
            let param_names = params.iter().map(|(n, _)| *n).collect();
            Ok(Value::Function(Function::Native {
                params: param_names,
                body: Rc::new(body.as_ref().clone()),
                env: env.clone(),
            }))
        }
        Expr::Block(defs) => {
            let frame = make_frame(defs, env, true);
            Ok(Value::Block(Rc::new(frame)))
        }
        Expr::ImmediateBlock(stmt) => interpret_statement(stmt, env),
        Expr::If { cond, cons, alt } => {
            let c = interpret(cond, env)?;
            if is_truthy(&c) {
                interpret(cons, env)
            } else {
                interpret(alt, env)
            }
        }
        Expr::Binary(op, l, r) => {
            let lv = interpret(l, env)?;
            let rv = interpret(r, env)?;
            apply_binop(*op, lv, rv, span)
        }
        Expr::Unary(op, e) => {
            let v = interpret(e, env)?;
            apply_unaryop(*op, v, span)
        }
        Expr::Call(callee, args) => {
            let cv = interpret(callee, env)?;
            let mut argvs = Vec::with_capacity(args.len());
            for arg in args {
                argvs.push(interpret(arg, env)?);
            }
            call_value(cv, argvs, span)
        }
        Expr::Access(obj, (name, name_span)) => {
            let ov = interpret(obj, env)?;
            let frame = match ov {
                Value::Block(f) => f,
                other => {
                    return Err(Diagnostic::new(
                        span.clone(),
                        format!("field access on {}", other.type_name()),
                        "not a block",
                    ))
                }
            };
            access_field(&frame, *name, name_span)
        }
        Expr::Index(arr, idx) => {
            let av = interpret(arr, env)?;
            let iv = interpret(idx, env)?;
            apply_index(av, iv, span)
        }
    }
}

fn access_field(frame: &Rc<Frame>, name: Symbol, span: &Span) -> EvalResult {
    let names = frame.names.as_ref().ok_or_else(|| {
        Diagnostic::new(
            span.clone(),
            "field access on non-record frame",
            "internal: frame has no field index",
        )
    })?;
    let slot = names.get(&name).ok_or_else(|| {
        Diagnostic::new(
            span.clone(),
            format!("no such field: {}", display(name)),
            "field not found",
        )
    })?;
    let bind = frame.binds[*slot as usize].clone();
    let env_at_def = Env(Some(frame.clone()));
    force(&env_at_def, &bind, span)
}

fn apply_index(arr: Value, idx: Value, span: &Span) -> EvalResult {
    match (arr, idx) {
        (Value::List(l), Value::Number(i)) => {
            let n = i as usize;
            l.get(n).cloned().ok_or_else(|| {
                Diagnostic::new(
                    span.clone(),
                    format!("index out of bounds: {}", n),
                    "list access",
                )
            })
        }
        (Value::Block(frame), Value::String(s)) => access_field(&frame, intern(&s), span),
        (a, i) => Err(Diagnostic::new(
            span.clone(),
            format!("cannot index {} by {}", a.type_name(), i.type_name()),
            "invalid index",
        )),
    }
}

pub fn call_value(callee: Value, args: Vec<Value>, span: &Span) -> EvalResult {
    match callee {
        Value::Function(Function::Native { params, body, env }) => {
            if params.len() != args.len() {
                return Err(Diagnostic::new(
                    span.clone(),
                    format!("expected {} arguments, got {}", params.len(), args.len()),
                    "argument count mismatch",
                ));
            }
            let mut binds = Vec::with_capacity(args.len());
            for (_p, a) in params.iter().zip(args.into_iter()) {
                binds.push(Rc::new(RefCell::new(BindState::Done(a))));
            }
            let frame = Frame {
                binds,
                names: None,
                parent: env,
            };
            interpret(&body, &Env(Some(Rc::new(frame))))
        }
        Value::Function(Function::Foreign(f)) => f(args, span),
        other => Err(Diagnostic::new(
            span.clone(),
            format!("cannot call {}", other.type_name()),
            "not a function",
        )),
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => *n != 0.0 && !n.is_nan(),
        _ => true,
    }
}

fn apply_binop(op: BinOp, l: Value, r: Value, span: &Span) -> EvalResult {
    match op {
        BinOp::Add => num_op(l, r, span, |a, b| a + b),
        BinOp::Sub => num_op(l, r, span, |a, b| a - b),
        BinOp::Mul => num_op(l, r, span, |a, b| a * b),
        BinOp::Div => num_op(l, r, span, |a, b| a / b),
        BinOp::Mod => num_op(l, r, span, |a, b| a % b),
        BinOp::Eq => Ok(Value::Bool(value_eq(&l, &r))),
        BinOp::Ne => Ok(Value::Bool(!value_eq(&l, &r))),
        BinOp::Gt => num_cmp(l, r, span, |a, b| a > b),
        BinOp::Lt => num_cmp(l, r, span, |a, b| a < b),
        BinOp::Ge => num_cmp(l, r, span, |a, b| a >= b),
        BinOp::Le => num_cmp(l, r, span, |a, b| a <= b),
    }
}

fn num_op(l: Value, r: Value, span: &Span, f: impl Fn(f64, f64) -> f64) -> EvalResult {
    Ok(Value::Number(f(into_number(l, span)?, into_number(r, span)?)))
}

fn num_cmp(l: Value, r: Value, span: &Span, f: impl Fn(f64, f64) -> bool) -> EvalResult {
    Ok(Value::Bool(f(into_number(l, span)?, into_number(r, span)?)))
}

fn into_number(v: Value, span: &Span) -> Result<f64, Diagnostic> {
    match v {
        Value::Number(n) => Ok(n),
        other => Err(Diagnostic::new(
            span.clone(),
            format!("expected number, got {}", other.type_name()),
            "type mismatch",
        )),
    }
}

fn value_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Number(a), Number(b)) => (a - b).abs() < f64::EPSILON,
        (Null, Null) => true,
        (Bool(a), Bool(b)) => a == b,
        (String(a), String(b)) => a == b,
        (List(a), List(b)) => {
            Rc::ptr_eq(a, b)
                || (a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| value_eq(x, y)))
        }
        _ => false,
    }
}

fn apply_unaryop(op: UnaryOp, v: Value, span: &Span) -> EvalResult {
    match op {
        UnaryOp::Neg => Ok(Value::Number(-into_number(v, span)?)),
        UnaryOp::Not => match v {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            other => Err(Diagnostic::new(
                span.clone(),
                format!("cannot apply ! to {}", other.type_name()),
                "type mismatch",
            )),
        },
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Bool(_) => "bool",
            Value::Null => "null",
            Value::List(_) => "list",
            Value::Function(_) => "function",
            Value::Block(_) => "block",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::List(v) => {
                let inner: Vec<String> = v.iter().map(|x| format!("{}", x)).collect();
                write!(f, "[{}]", inner.join(", "))
            }
            Value::Function(_) => write!(f, "[function]"),
            Value::Block(b) => {
                let mut names: Vec<&str> = b
                    .names
                    .as_ref()
                    .map(|n| n.keys().map(|s| display(*s)).collect())
                    .unwrap_or_default();
                names.sort();
                write!(f, "{{{}}}", names.join(", "))
            }
        }
    }
}
