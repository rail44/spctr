use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::Span;
use crate::types::*;

pub struct TypeCheckResult {
    pub program_type: Type,
    pub warnings: Vec<Diagnostic>,
}

pub fn check(stmt: &Statement, root_types: &[Type]) -> TypeCheckResult {
    let mut inferer = Inferer::new();
    let mut env = TypeEnv {
        frames: vec![root_types.to_vec()],
    };
    let program_type = inferer.infer_statement(stmt, &mut env);
    let resolved = program_type.apply(&inferer.subst);
    TypeCheckResult {
        program_type: resolved,
        warnings: inferer.warnings,
    }
}

struct TypeEnv {
    frames: Vec<Vec<Type>>,
}

impl TypeEnv {
    fn lookup(&self, depth: u32, slot: u32) -> Option<&Type> {
        let idx = self.frames.len().checked_sub(1)?.checked_sub(depth as usize)?;
        self.frames.get(idx)?.get(slot as usize)
    }
}

struct Inferer {
    next_var: u32,
    subst: Subst,
    warnings: Vec<Diagnostic>,
}

impl Inferer {
    fn new() -> Self {
        Self {
            next_var: 0,
            subst: Subst::new(),
            warnings: Vec::new(),
        }
    }

    fn fresh(&mut self) -> Type {
        let v = TypeVar(self.next_var);
        self.next_var += 1;
        Type::Var(v)
    }

    fn unify(&mut self, a: &Type, b: &Type, span: &Span) {
        let a = a.apply(&self.subst);
        let b = b.apply(&self.subst);
        if let Err(reason) = self.unify_inner(&a, &b) {
            self.warnings.push(Diagnostic::new(
                span.clone(),
                format!("type mismatch: {} vs {}", a, b),
                reason,
            ));
        }
    }

    fn unify_inner(&mut self, a: &Type, b: &Type) -> Result<(), String> {
        match (a, b) {
            (Type::Any, _) | (_, Type::Any) => Ok(()),
            (Type::Number, Type::Number)
            | (Type::String, Type::String)
            | (Type::Bool, Type::Bool)
            | (Type::Null, Type::Null) => Ok(()),
            (Type::Var(v), t) | (t, Type::Var(v)) => {
                if let Type::Var(v2) = t {
                    if v == v2 {
                        return Ok(());
                    }
                }
                if t.contains(*v) {
                    return Err("infinite type (occurs check)".into());
                }
                self.subst.insert(*v, t.clone());
                Ok(())
            }
            (Type::Fn(a1, r1), Type::Fn(a2, r2)) => {
                if a1.len() != a2.len() {
                    return Err(format!(
                        "function arity: expected {} args, got {}",
                        a1.len(),
                        a2.len()
                    ));
                }
                for (x, y) in a1.iter().zip(a2.iter()) {
                    self.unify_inner(x, y)?;
                }
                self.unify_inner(r1, r2)
            }
            (Type::List(t1), Type::List(t2)) => self.unify_inner(t1, t2),
            (a, b) => Err(format!("incompatible: {} vs {}", a, b)),
        }
    }

    fn infer_statement(&mut self, stmt: &Statement, env: &mut TypeEnv) -> Type {
        let frame: Vec<Type> = stmt.definitions.iter().map(|_| self.fresh()).collect();
        env.frames.push(frame);

        for (i, (_, body)) in stmt.definitions.iter().enumerate() {
            let bt = self.infer(body, env);
            let slot_t = env.frames.last().unwrap()[i].clone();
            self.unify(&slot_t, &bt, &body.1);
            // refresh frame slot with substitutions applied
            let resolved = slot_t.apply(&self.subst);
            env.frames.last_mut().unwrap()[i] = resolved;
        }

        let body_t = self.infer(&stmt.body, env);
        env.frames.pop();
        body_t
    }

    fn infer(&mut self, expr: &Spanned<Expr>, env: &mut TypeEnv) -> Type {
        match &expr.0 {
            Expr::Number(_) => Type::Number,
            Expr::String(_) => Type::String,
            Expr::Bool(_) => Type::Bool,
            Expr::Null => Type::Null,
            Expr::Variable(var) => match var.resolved.get() {
                Some(bref) => env
                    .lookup(bref.depth, bref.slot)
                    .cloned()
                    .map(|t| t.apply(&self.subst))
                    .unwrap_or(Type::Any),
                None => Type::Any,
            },
            Expr::List(items) => {
                let elem = self.fresh();
                for it in items {
                    let t = self.infer(it, env);
                    self.unify(&elem, &t, &it.1);
                }
                Type::List(Box::new(elem.apply(&self.subst)))
            }
            Expr::Function(params, body) => {
                let param_types: Vec<Type> = params.iter().map(|_| self.fresh()).collect();
                env.frames.push(param_types.clone());
                let ret_type = self.infer(body, env);
                env.frames.pop();
                let ret_resolved = ret_type.apply(&self.subst);
                let params_resolved: Vec<Type> =
                    param_types.iter().map(|t| t.apply(&self.subst)).collect();
                Type::Fn(params_resolved, Box::new(ret_resolved))
            }
            Expr::Block(_) => Type::Any,
            Expr::ImmediateBlock(stmt) => self.infer_statement(stmt, env),
            Expr::If { cond, cons, alt } => {
                let ct = self.infer(cond, env);
                self.unify(&ct, &Type::Bool, &cond.1);
                let at = self.infer(cons, env);
                let bt = self.infer(alt, env);
                self.unify(&at, &bt, &expr.1);
                at.apply(&self.subst)
            }
            Expr::Binary(op, l, r) => {
                let lt = self.infer(l, env);
                let rt = self.infer(r, env);
                self.infer_binop(*op, &lt, &rt, &l.1, &r.1)
            }
            Expr::Unary(op, e) => {
                let t = self.infer(e, env);
                match op {
                    UnaryOp::Neg => {
                        self.unify(&t, &Type::Number, &e.1);
                        Type::Number
                    }
                    UnaryOp::Not => {
                        self.unify(&t, &Type::Bool, &e.1);
                        Type::Bool
                    }
                }
            }
            Expr::Call(callee, args) => {
                let ct = self.infer(callee, env);
                let arg_ts: Vec<Type> = args.iter().map(|a| self.infer(a, env)).collect();
                let ret = self.fresh();
                let expected = Type::Fn(arg_ts, Box::new(ret.clone()));
                self.unify(&ct, &expected, &expr.1);
                ret.apply(&self.subst)
            }
            Expr::Access(_, _) => Type::Any,
            Expr::Index(_, _) => Type::Any,
        }
    }

    fn infer_binop(
        &mut self,
        op: BinOp,
        lt: &Type,
        rt: &Type,
        ls: &Span,
        rs: &Span,
    ) -> Type {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                self.unify(lt, &Type::Number, ls);
                self.unify(rt, &Type::Number, rs);
                Type::Number
            }
            BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => {
                self.unify(lt, &Type::Number, ls);
                self.unify(rt, &Type::Number, rs);
                Type::Bool
            }
            BinOp::Eq | BinOp::Ne => {
                self.unify(lt, rt, rs);
                Type::Bool
            }
            BinOp::And | BinOp::Or => {
                // be permissive: spctr returns lhs when short-circuit fires (might be non-bool)
                self.unify(lt, &Type::Bool, ls);
                self.unify(rt, &Type::Bool, rs);
                Type::Bool
            }
        }
    }
}
