use crate::diag::Diagnostic;
use crate::eval::{make_block, make_foreign, type_name, Value};
use crate::lexer::Span;
use std::collections::HashMap;
use std::rc::Rc;

pub fn module() -> Value {
    let mut bindings = HashMap::new();
    bindings.insert("concat".to_string(), make_foreign(concat));
    make_block(bindings)
}

fn concat(mut args: Vec<Value>, span: &Span) -> Result<Value, Diagnostic> {
    if args.len() != 2 {
        return Err(Diagnostic::new(
            span.clone(),
            "String.concat: arity mismatch",
            format!("expected 2 args, got {}", args.len()),
        ));
    }
    let b = args.pop().unwrap();
    let a = args.pop().unwrap();
    match (&a, &b) {
        (Value::String(x), Value::String(y)) => {
            Ok(Value::String(Rc::new(format!("{}{}", x, y))))
        }
        _ => Err(Diagnostic::new(
            span.clone(),
            "String.concat: arguments must be strings",
            format!("got {} and {}", type_name(&a), type_name(&b)),
        )),
    }
}
