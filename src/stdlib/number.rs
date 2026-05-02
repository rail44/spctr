use crate::diag::Diagnostic;
use crate::interp::{BindState, Env, EvalResult, Frame, Function, Value};
use crate::lexer::Span;
use crate::symbol::intern;
use crate::types::{Scheme, Type};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn ty() -> Type {
    let mono = |ty: Type| Scheme { vars: vec![], ty };
    let n_to_n = || Type::Fn(vec![Type::Number], Box::new(Type::Number));
    let nn_to_n = || Type::Fn(vec![Type::Number, Type::Number], Box::new(Type::Number));

    Type::Module(vec![
        (
            intern("toString"),
            mono(Type::Fn(vec![Type::Number], Box::new(Type::String))),
        ),
        (
            intern("parse"),
            mono(Type::Fn(vec![Type::String], Box::new(Type::Number))),
        ),
        (intern("abs"), mono(n_to_n())),
        (intern("floor"), mono(n_to_n())),
        (intern("ceil"), mono(n_to_n())),
        (intern("round"), mono(n_to_n())),
        (intern("sqrt"), mono(n_to_n())),
        (intern("pow"), mono(nn_to_n())),
        (intern("min"), mono(nn_to_n())),
        (intern("max"), mono(nn_to_n())),
    ])
}

pub fn module() -> Value {
    let entries: Vec<(&str, fn(Vec<Value>, &Span) -> EvalResult)> = vec![
        ("toString", to_string),
        ("parse", parse),
        ("abs", abs),
        ("floor", floor),
        ("ceil", ceil),
        ("round", round),
        ("sqrt", sqrt),
        ("pow", pow),
        ("min", min),
        ("max", max),
    ];

    let mut binds = Vec::with_capacity(entries.len());
    let mut names = HashMap::with_capacity(entries.len());
    for (i, (name, f)) in entries.into_iter().enumerate() {
        binds.push(Rc::new(RefCell::new(BindState::Done(Value::Function(
            Function::Foreign(Rc::new(f)),
        )))));
        names.insert(intern(name), i as u32);
    }

    Value::Block(Rc::new(Frame {
        binds,
        names: Some(names),
        parent: Env::empty(),
    }))
}

fn arity(args: &[Value], expected: usize, name: &str, span: &Span) -> Result<(), Diagnostic> {
    if args.len() != expected {
        Err(Diagnostic::new(
            span.clone(),
            format!("Number.{} expects {} arguments, got {}", name, expected, args.len()),
            "argument count",
        ))
    } else {
        Ok(())
    }
}

fn into_number(v: &Value, span: &Span) -> Result<f64, Diagnostic> {
    match v {
        Value::Number(n) => Ok(*n),
        other => Err(Diagnostic::new(
            span.clone(),
            format!("expected number, got {}", other.type_name()),
            "type mismatch",
        )),
    }
}

fn into_string(v: &Value, span: &Span) -> Result<Rc<String>, Diagnostic> {
    match v {
        Value::String(s) => Ok(s.clone()),
        other => Err(Diagnostic::new(
            span.clone(),
            format!("expected string, got {}", other.type_name()),
            "type mismatch",
        )),
    }
}

fn to_string(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "toString", span)?;
    let n = into_number(&args[0], span)?;
    Ok(Value::String(Rc::new(format!("{}", n))))
}

fn parse(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "parse", span)?;
    let s = into_string(&args[0], span)?;
    s.trim().parse::<f64>().map(Value::Number).map_err(|_| {
        Diagnostic::new(
            span.clone(),
            format!("cannot parse {:?} as number", s.as_str()),
            "invalid number",
        )
    })
}

fn unary_num(
    args: Vec<Value>,
    span: &Span,
    name: &str,
    f: impl Fn(f64) -> f64,
) -> EvalResult {
    arity(&args, 1, name, span)?;
    Ok(Value::Number(f(into_number(&args[0], span)?)))
}

fn binary_num(
    args: Vec<Value>,
    span: &Span,
    name: &str,
    f: impl Fn(f64, f64) -> f64,
) -> EvalResult {
    arity(&args, 2, name, span)?;
    Ok(Value::Number(f(
        into_number(&args[0], span)?,
        into_number(&args[1], span)?,
    )))
}

fn abs(args: Vec<Value>, span: &Span) -> EvalResult {
    unary_num(args, span, "abs", f64::abs)
}
fn floor(args: Vec<Value>, span: &Span) -> EvalResult {
    unary_num(args, span, "floor", f64::floor)
}
fn ceil(args: Vec<Value>, span: &Span) -> EvalResult {
    unary_num(args, span, "ceil", f64::ceil)
}
fn round(args: Vec<Value>, span: &Span) -> EvalResult {
    unary_num(args, span, "round", f64::round)
}
fn sqrt(args: Vec<Value>, span: &Span) -> EvalResult {
    unary_num(args, span, "sqrt", f64::sqrt)
}
fn pow(args: Vec<Value>, span: &Span) -> EvalResult {
    binary_num(args, span, "pow", f64::powf)
}
fn min(args: Vec<Value>, span: &Span) -> EvalResult {
    binary_num(args, span, "min", f64::min)
}
fn max(args: Vec<Value>, span: &Span) -> EvalResult {
    binary_num(args, span, "max", f64::max)
}
