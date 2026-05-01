use crate::ast::*;
use crate::diag::Diagnostic;
use crate::symbol::{display, intern, Symbol};
use std::collections::HashMap;

pub fn resolve(stmt: &Statement, root_names: &[&str]) -> Result<(), Diagnostic> {
    let mut resolver = Resolver { scopes: Vec::new() };
    let mut root_scope: HashMap<Symbol, u32> = HashMap::new();
    for (i, name) in root_names.iter().enumerate() {
        root_scope.insert(intern(name), i as u32);
    }
    resolver.scopes.push(root_scope);
    resolver.statement(stmt)
}

struct Resolver {
    scopes: Vec<HashMap<Symbol, u32>>,
}

impl Resolver {
    fn statement(&mut self, stmt: &Statement) -> Result<(), Diagnostic> {
        let mut scope: HashMap<Symbol, u32> = HashMap::new();
        for (i, ((name, _), _)) in stmt.definitions.iter().enumerate() {
            scope.insert(*name, i as u32);
        }
        self.scopes.push(scope);
        for (_, body) in &stmt.definitions {
            self.expr(body)?;
        }
        self.expr(&stmt.body)?;
        self.scopes.pop();
        Ok(())
    }

    fn expr(&mut self, expr: &Spanned<Expr>) -> Result<(), Diagnostic> {
        match &expr.0 {
            Expr::Number(_) | Expr::String(_) | Expr::Null => Ok(()),
            Expr::Variable(var) => {
                for (depth, scope) in self.scopes.iter().rev().enumerate() {
                    if let Some(slot) = scope.get(&var.name) {
                        var.resolved.set(Some(BindRef {
                            depth: depth as u32,
                            slot: *slot,
                        }));
                        return Ok(());
                    }
                }
                Err(Diagnostic::new(
                    expr.1.clone(),
                    format!("undefined variable: {}", display(var.name)),
                    "not found in scope",
                ))
            }
            Expr::List(items) => {
                for item in items {
                    self.expr(item)?;
                }
                Ok(())
            }
            Expr::Function(args, body) => {
                let mut scope: HashMap<Symbol, u32> = HashMap::new();
                for (i, (name, _)) in args.iter().enumerate() {
                    scope.insert(*name, i as u32);
                }
                self.scopes.push(scope);
                self.expr(body)?;
                self.scopes.pop();
                Ok(())
            }
            Expr::Block(defs) => {
                let mut scope: HashMap<Symbol, u32> = HashMap::new();
                for (i, ((name, _), _)) in defs.iter().enumerate() {
                    scope.insert(*name, i as u32);
                }
                self.scopes.push(scope);
                for (_, body) in defs {
                    self.expr(body)?;
                }
                self.scopes.pop();
                Ok(())
            }
            Expr::ImmediateBlock(stmt) => self.statement(stmt),
            Expr::If { cond, cons, alt } => {
                self.expr(cond)?;
                self.expr(cons)?;
                self.expr(alt)?;
                Ok(())
            }
            Expr::Binary(_, l, r) => {
                self.expr(l)?;
                self.expr(r)?;
                Ok(())
            }
            Expr::Unary(_, e) => self.expr(e),
            Expr::Call(callee, args) => {
                self.expr(callee)?;
                for arg in args {
                    self.expr(arg)?;
                }
                Ok(())
            }
            Expr::Access(obj, _) => self.expr(obj),
            Expr::Index(arr, idx) => {
                self.expr(arr)?;
                self.expr(idx)?;
                Ok(())
            }
        }
    }
}
