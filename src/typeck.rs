use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::Span;
use crate::symbol::display;
use crate::types::*;
use std::collections::{HashMap, HashSet};

pub struct TypeCheckResult {
    pub program_type: Type,
    pub warnings: Vec<Diagnostic>,
}

pub fn check(stmt: &Statement, root_types: &[Type]) -> TypeCheckResult {
    let mut inferer = Inferer::new();
    let mut env = TypeEnv {
        frames: vec![root_types.iter().cloned().map(Scheme::mono).collect()],
    };
    let program_type = inferer.infer_statement(stmt, &mut env);
    let resolved = program_type.apply(&inferer.subst);
    TypeCheckResult {
        program_type: resolved,
        warnings: inferer.warnings,
    }
}

struct TypeEnv {
    frames: Vec<Vec<Scheme>>,
}

impl TypeEnv {
    fn lookup(&self, depth: u32, slot: u32) -> Option<&Scheme> {
        let idx = self.frames.len().checked_sub(1)?.checked_sub(depth as usize)?;
        self.frames.get(idx)?.get(slot as usize)
    }

    fn free_vars(&self) -> HashSet<TypeVar> {
        let mut set = HashSet::new();
        for frame in &self.frames {
            for sch in frame {
                sch.free_vars(&mut set);
            }
        }
        set
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

    fn fresh_var(&mut self) -> TypeVar {
        let v = TypeVar(self.next_var);
        self.next_var += 1;
        v
    }

    fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let mut subst = HashMap::new();
        for v in &scheme.vars {
            subst.insert(*v, Type::Var(self.fresh_var()));
        }
        scheme.ty.apply(&subst)
    }

    fn generalize(&self, env_vars: &HashSet<TypeVar>, ty: &Type) -> Scheme {
        let ty = ty.apply(&self.subst);
        let mut vars = HashSet::new();
        ty.free_vars(&mut vars);
        let quantified: Vec<TypeVar> = vars.into_iter().filter(|v| !env_vars.contains(v)).collect();
        Scheme {
            vars: quantified,
            ty,
        }
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
                if let Some(existing) = self.subst.get(v).cloned() {
                    return self.unify_inner(&existing, t);
                }
                let t_resolved = t.apply(&self.subst);
                if let Type::Var(v2) = &t_resolved {
                    if v == v2 {
                        return Ok(());
                    }
                }
                if t_resolved.contains(*v) {
                    return Err("infinite type (occurs check)".into());
                }
                self.subst.insert(*v, t_resolved);
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
                    let x = x.apply(&self.subst);
                    let y = y.apply(&self.subst);
                    self.unify_inner(&x, &y)?;
                }
                let r1 = r1.apply(&self.subst);
                let r2 = r2.apply(&self.subst);
                self.unify_inner(&r1, &r2)
            }
            (Type::List(t1), Type::List(t2)) => self.unify_inner(t1, t2),
            (Type::Record(f1), Type::Record(f2)) => {
                if f1.len() != f2.len() {
                    return Err(format!("record fields: {} vs {}", f1.len(), f2.len()));
                }
                let m1: HashMap<_, _> = f1.iter().map(|(n, t)| (*n, t.clone())).collect();
                for (k2, v2) in f2 {
                    match m1.get(k2) {
                        Some(v1) => self.unify_inner(v1, v2)?,
                        None => return Err(format!("missing field '{}'", display(*k2))),
                    }
                }
                Ok(())
            }
            (Type::Module(f1), Type::Module(f2)) => {
                if f1.len() != f2.len() {
                    return Err(format!("module fields: {} vs {}", f1.len(), f2.len()));
                }
                let m1: HashMap<_, _> = f1.iter().map(|(n, s)| (*n, s.ty.clone())).collect();
                for (k2, sch2) in f2 {
                    match m1.get(k2) {
                        Some(t1) => self.unify_inner(t1, &sch2.ty)?,
                        None => return Err(format!("missing field '{}'", display(*k2))),
                    }
                }
                Ok(())
            }
            (a, b) => Err(format!("incompatible: {} vs {}", a, b)),
        }
    }

    fn infer_statement(&mut self, stmt: &Statement, env: &mut TypeEnv) -> Type {
        // Pre-allocate fresh-var slots so siblings can reference each other
        let frame: Vec<Scheme> = stmt
            .definitions
            .iter()
            .map(|_| Scheme::mono(self.fresh()))
            .collect();
        env.frames.push(frame);

        // Per-bind: infer body, unify with slot, generalize this slot
        for (i, (_, body)) in stmt.definitions.iter().enumerate() {
            let slot_ty = env.frames.last().unwrap()[i].ty.clone();
            let bt = self.infer(body, env);
            self.unify(&slot_ty, &bt, &body.1);
            let outer_vars = self.outer_vars_excluding(env, i);
            let gen = self.generalize(&outer_vars, &slot_ty);
            env.frames.last_mut().unwrap()[i] = gen;
        }

        let body_t = self.infer(&stmt.body, env);
        env.frames.pop();
        body_t
    }

    fn outer_vars_excluding(&self, env: &TypeEnv, exclude_idx: usize) -> HashSet<TypeVar> {
        let mut set = HashSet::new();
        let last = env.frames.len() - 1;
        for f in &env.frames[..last] {
            for sch in f {
                sch.free_vars(&mut set);
            }
        }
        for (j, sch) in env.frames[last].iter().enumerate() {
            if j != exclude_idx {
                sch.free_vars(&mut set);
            }
        }
        set
    }

    fn infer(&mut self, expr: &Spanned<Expr>, env: &mut TypeEnv) -> Type {
        match &expr.0 {
            Expr::Number(_) => Type::Number,
            Expr::String(_) => Type::String,
            Expr::Bool(_) => Type::Bool,
            Expr::Null => Type::Null,
            Expr::Variable(var) => match var.resolved.get() {
                Some(bref) => match env.lookup(bref.depth, bref.slot) {
                    Some(sch) => {
                        let sch = sch.clone();
                        self.instantiate(&sch).apply(&self.subst)
                    }
                    None => Type::Any,
                },
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
                let frame: Vec<Scheme> =
                    param_types.iter().cloned().map(Scheme::mono).collect();
                env.frames.push(frame);
                let ret_type = self.infer(body, env);
                env.frames.pop();
                let ret_resolved = ret_type.apply(&self.subst);
                let params_resolved: Vec<Type> =
                    param_types.iter().map(|t| t.apply(&self.subst)).collect();
                Type::Fn(params_resolved, Box::new(ret_resolved))
            }
            Expr::Block(defs) => {
                let frame: Vec<Scheme> = defs.iter().map(|_| Scheme::mono(self.fresh())).collect();
                env.frames.push(frame);

                for (i, (_, body)) in defs.iter().enumerate() {
                    let slot_ty = env.frames.last().unwrap()[i].ty.clone();
                    let bt = self.infer(body, env);
                    self.unify(&slot_ty, &bt, &body.1);
                    let outer_vars = self.outer_vars_excluding(env, i);
                    let gen = self.generalize(&outer_vars, &slot_ty);
                    env.frames.last_mut().unwrap()[i] = gen;
                }

                let this_frame = env.frames.pop().unwrap();
                let fields: Vec<_> = defs
                    .iter()
                    .zip(this_frame.iter())
                    .map(|(((name, _), _), sch)| (*name, sch.ty.apply(&self.subst)))
                    .collect();
                Type::Record(fields)
            }
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
            Expr::Access(obj, (name, name_span)) => {
                let obj_t = self.infer(obj, env).apply(&self.subst);
                match &obj_t {
                    Type::Any => Type::Any,
                    Type::Var(_) => Type::Any,
                    Type::Record(fields) => fields
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, t)| t.clone())
                        .unwrap_or_else(|| {
                            self.warnings.push(Diagnostic::new(
                                name_span.clone(),
                                format!("no field '{}' on {}", display(*name), obj_t),
                                "field not found in record",
                            ));
                            Type::Any
                        }),
                    Type::Module(fields) => fields
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, sch)| {
                            let sch = sch.clone();
                            self.instantiate(&sch)
                        })
                        .unwrap_or_else(|| {
                            self.warnings.push(Diagnostic::new(
                                name_span.clone(),
                                format!("no field '{}' on {}", display(*name), obj_t),
                                "field not found in module",
                            ));
                            Type::Any
                        }),
                    _ => {
                        self.warnings.push(Diagnostic::new(
                            expr.1.clone(),
                            format!("field access on {}", obj_t),
                            "expected record",
                        ));
                        Type::Any
                    }
                }
            }
            Expr::Index(arr, idx) => {
                let arr_t = self.infer(arr, env).apply(&self.subst);
                let idx_t = self.infer(idx, env).apply(&self.subst);
                match &arr_t {
                    Type::Any => Type::Any,
                    Type::Var(_) => Type::Any,
                    Type::List(elem) => {
                        self.unify(&idx_t, &Type::Number, &idx.1);
                        elem.as_ref().apply(&self.subst)
                    }
                    _ => {
                        self.warnings.push(Diagnostic::new(
                            expr.1.clone(),
                            format!("indexing {}", arr_t),
                            "expected list",
                        ));
                        Type::Any
                    }
                }
            }
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
                self.unify(lt, &Type::Bool, ls);
                self.unify(rt, &Type::Bool, rs);
                Type::Bool
            }
        }
    }
}
