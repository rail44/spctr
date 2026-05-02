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
    Type::Module(vec![
        (
            intern("length"),
            mono(Type::Fn(vec![Type::String], Box::new(Type::Number))),
        ),
        (
            intern("concat"),
            mono(Type::Fn(
                vec![Type::String, Type::String],
                Box::new(Type::String),
            )),
        ),
        (
            intern("split"),
            mono(Type::Fn(
                vec![Type::String, Type::String],
                Box::new(Type::List(Box::new(Type::String))),
            )),
        ),
        (
            intern("contains"),
            mono(Type::Fn(
                vec![Type::String, Type::String],
                Box::new(Type::Bool),
            )),
        ),
        (
            intern("to_lower"),
            mono(Type::Fn(vec![Type::String], Box::new(Type::String))),
        ),
        (
            intern("to_upper"),
            mono(Type::Fn(vec![Type::String], Box::new(Type::String))),
        ),
    ])
}

pub fn module() -> Value {
    let entries: Vec<(&str, fn(Vec<Value>, &Span) -> EvalResult)> = vec![
        ("length", length),
        ("concat", concat),
        ("split", split),
        ("contains", contains),
        ("to_lower", to_lower),
        ("to_upper", to_upper),
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
            format!("String.{} expects {} arguments, got {}", name, expected, args.len()),
            "argument count",
        ))
    } else {
        Ok(())
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

fn length(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "length", span)?;
    let s = into_string(&args[0], span)?;
    Ok(Value::Number(s.chars().count() as f64))
}

fn concat(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "concat", span)?;
    let a = into_string(&args[0], span)?;
    let b = into_string(&args[1], span)?;
    Ok(Value::String(Rc::new(format!("{}{}", a, b))))
}

fn split(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "split", span)?;
    let s = into_string(&args[0], span)?;
    let sep = into_string(&args[1], span)?;
    let parts: Vec<Value> = if sep.is_empty() {
        s.chars()
            .map(|c| Value::String(Rc::new(c.to_string())))
            .collect()
    } else {
        s.split(sep.as_str())
            .map(|p| Value::String(Rc::new(p.to_string())))
            .collect()
    };
    Ok(Value::List(Rc::new(parts)))
}

fn contains(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "contains", span)?;
    let s = into_string(&args[0], span)?;
    let needle = into_string(&args[1], span)?;
    Ok(Value::Bool(s.contains(needle.as_str())))
}

fn to_lower(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "to_lower", span)?;
    let s = into_string(&args[0], span)?;
    Ok(Value::String(Rc::new(s.to_lowercase())))
}

fn to_upper(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "to_upper", span)?;
    let s = into_string(&args[0], span)?;
    Ok(Value::String(Rc::new(s.to_uppercase())))
}
