use crate::diag::Diagnostic;
use crate::interp::{call_value, BindState, Env, EvalResult, Frame, Function, Value};
use crate::lexer::Span;
use crate::symbol::intern;
use crate::types::{Scheme, Type, TypeVar};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn ty() -> Type {
    let alpha = TypeVar(0);
    let beta = TypeVar(1);
    let list_a = || Type::List(Box::new(Type::Var(alpha)));
    let list_b = || Type::List(Box::new(Type::Var(beta)));
    let var_a = || Type::Var(alpha);
    let var_b = || Type::Var(beta);
    let scheme_a = |ty: Type| Scheme {
        vars: vec![alpha],
        ty,
    };
    let scheme_ab = |ty: Type| Scheme {
        vars: vec![alpha, beta],
        ty,
    };

    Type::Module(vec![
        (
            intern("range"),
            Scheme {
                vars: vec![],
                ty: Type::Fn(
                    vec![Type::Number, Type::Number],
                    Box::new(Type::List(Box::new(Type::Number))),
                ),
            },
        ),
        (
            intern("length"),
            scheme_a(Type::Fn(vec![list_a()], Box::new(Type::Number))),
        ),
        (
            intern("head"),
            scheme_a(Type::Fn(vec![list_a()], Box::new(var_a()))),
        ),
        (
            intern("tail"),
            scheme_a(Type::Fn(vec![list_a()], Box::new(list_a()))),
        ),
        (
            intern("concat"),
            scheme_a(Type::Fn(vec![list_a(), list_a()], Box::new(list_a()))),
        ),
        (
            intern("take"),
            scheme_a(Type::Fn(vec![list_a(), Type::Number], Box::new(list_a()))),
        ),
        (
            intern("drop"),
            scheme_a(Type::Fn(vec![list_a(), Type::Number], Box::new(list_a()))),
        ),
        (
            intern("map"),
            scheme_ab(Type::Fn(
                vec![list_a(), Type::Fn(vec![var_a()], Box::new(var_b()))],
                Box::new(list_b()),
            )),
        ),
        (
            intern("filter"),
            scheme_a(Type::Fn(
                vec![list_a(), Type::Fn(vec![var_a()], Box::new(Type::Bool))],
                Box::new(list_a()),
            )),
        ),
        (
            intern("reduce"),
            scheme_ab(Type::Fn(
                vec![
                    list_a(),
                    var_b(),
                    Type::Fn(vec![var_b(), var_a()], Box::new(var_b())),
                ],
                Box::new(var_b()),
            )),
        ),
    ])
}

pub fn module() -> Value {
    let entries: Vec<(&str, fn(Vec<Value>, &Span) -> EvalResult)> = vec![
        ("range", range),
        ("length", length),
        ("head", head),
        ("tail", tail),
        ("concat", concat),
        ("take", take),
        ("drop", drop_),
        ("map", map),
        ("filter", filter),
        ("reduce", reduce),
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
            format!("List.{} expects {} arguments, got {}", name, expected, args.len()),
            "argument count",
        ))
    } else {
        Ok(())
    }
}

fn into_list(v: &Value, span: &Span) -> Result<Rc<Vec<Value>>, Diagnostic> {
    match v {
        Value::List(l) => Ok(l.clone()),
        other => Err(Diagnostic::new(
            span.clone(),
            format!("expected list, got {}", other.type_name()),
            "type mismatch",
        )),
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

fn range(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "range", span)?;
    let from = into_number(&args[0], span)? as i64;
    let to = into_number(&args[1], span)? as i64;
    let xs: Vec<Value> = (from..to).map(|i| Value::Number(i as f64)).collect();
    Ok(Value::List(Rc::new(xs)))
}

fn length(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "length", span)?;
    let xs = into_list(&args[0], span)?;
    Ok(Value::Number(xs.len() as f64))
}

fn head(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "head", span)?;
    let xs = into_list(&args[0], span)?;
    xs.first().cloned().ok_or_else(|| {
        Diagnostic::new(
            span.clone(),
            "List.head on empty list",
            "no first element",
        )
    })
}

fn tail(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 1, "tail", span)?;
    let xs = into_list(&args[0], span)?;
    if xs.is_empty() {
        return Err(Diagnostic::new(
            span.clone(),
            "List.tail on empty list",
            "no tail",
        ));
    }
    Ok(Value::List(Rc::new(xs[1..].to_vec())))
}

fn concat(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "concat", span)?;
    let a = into_list(&args[0], span)?;
    let b = into_list(&args[1], span)?;
    let mut result: Vec<Value> = (*a).clone();
    result.extend(b.iter().cloned());
    Ok(Value::List(Rc::new(result)))
}

fn take(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "take", span)?;
    let xs = into_list(&args[0], span)?;
    let n = into_number(&args[1], span)? as usize;
    let n = n.min(xs.len());
    Ok(Value::List(Rc::new(xs[..n].to_vec())))
}

fn drop_(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "drop", span)?;
    let xs = into_list(&args[0], span)?;
    let n = into_number(&args[1], span)? as usize;
    let n = n.min(xs.len());
    Ok(Value::List(Rc::new(xs[n..].to_vec())))
}

fn map(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "map", span)?;
    let xs = into_list(&args[0], span)?;
    let f = args[1].clone();
    let mut result = Vec::with_capacity(xs.len());
    for x in xs.iter() {
        result.push(call_value(f.clone(), vec![x.clone()], span)?);
    }
    Ok(Value::List(Rc::new(result)))
}

fn filter(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 2, "filter", span)?;
    let xs = into_list(&args[0], span)?;
    let f = args[1].clone();
    let mut result = Vec::new();
    for x in xs.iter() {
        let v = call_value(f.clone(), vec![x.clone()], span)?;
        match v {
            Value::Bool(true) => result.push(x.clone()),
            Value::Bool(false) => {}
            other => {
                return Err(Diagnostic::new(
                    span.clone(),
                    format!("List.filter predicate returned {}", other.type_name()),
                    "expected bool",
                ))
            }
        }
    }
    Ok(Value::List(Rc::new(result)))
}

fn reduce(args: Vec<Value>, span: &Span) -> EvalResult {
    arity(&args, 3, "reduce", span)?;
    let xs = into_list(&args[0], span)?;
    let mut acc = args[1].clone();
    let f = args[2].clone();
    for x in xs.iter() {
        acc = call_value(f.clone(), vec![acc, x.clone()], span)?;
    }
    Ok(acc)
}
