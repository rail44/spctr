use crate::diag::Diagnostic;
use crate::interp::{run_eager as interp_run, EvalResult, Value};
use crate::lexer::Span;
use crate::types::{Scheme, Type};
use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    static CURRENT_DIR: RefCell<PathBuf> = RefCell::new(
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    );
}

pub fn ty() -> Scheme {
    Scheme {
        vars: vec![],
        ty: Type::Fn(vec![Type::String], Box::new(Type::Any)),
    }
}

pub fn set_current_dir(dir: PathBuf) {
    CURRENT_DIR.with(|cell| *cell.borrow_mut() = dir);
}

pub fn import(args: Vec<Value>, span: &Span) -> EvalResult {
    if args.len() != 1 {
        return Err(Diagnostic::new(
            span.clone(),
            format!("import expects 1 argument, got {}", args.len()),
            "argument count",
        ));
    }
    let raw_path = match &args[0] {
        Value::String(s) => s.clone(),
        other => {
            return Err(Diagnostic::new(
                span.clone(),
                format!("import expects a string path, got {}", other.type_name()),
                "type mismatch",
            ))
        }
    };

    let candidate = PathBuf::from(raw_path.as_str());
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        CURRENT_DIR.with(|cell| cell.borrow().join(&candidate))
    };

    let source = std::fs::read_to_string(&resolved).map_err(|e| {
        Diagnostic::new(
            span.clone(),
            format!("cannot read {}: {}", resolved.display(), e),
            "import error",
        )
    })?;

    let ast = crate::parser::parse(&source).map_err(|errs| {
        let summary = errs
            .iter()
            .map(|d| format!("{}: {}", d.message, d.label))
            .collect::<Vec<_>>()
            .join("; ");
        Diagnostic::new(
            span.clone(),
            format!("parse error in {}: {}", resolved.display(), summary),
            "import error",
        )
    })?;

    crate::resolver::resolve(&ast, &crate::interp::ROOT_NAMES).map_err(|d| {
        Diagnostic::new(
            span.clone(),
            format!("resolve error in {}: {}", resolved.display(), d.message),
            "import error",
        )
    })?;

    let imported_dir = resolved
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let prev_dir = CURRENT_DIR.with(|cell| cell.borrow().clone());
    set_current_dir(imported_dir);
    let result = interp_run(&ast);
    set_current_dir(prev_dir);
    result
}
