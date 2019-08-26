use crate::types::Type;
use crate::Unevaluated;
use crate::{token, Env};
use failure::format_err;
use std::convert::TryInto;
use std::iter::IntoIterator;

pub trait Evaluable {
    fn eval(self, env: &Env) -> Result<Type, failure::Error>;
}

pub fn eval_source(mut source: token::Source, env: &Env) -> Result<Type, failure::Error> {
    let mut child = Env::new(source.bind_map, Default::default());
    if let Some(base) = source.base {
        let base: Env = env.get_value(&base)?.try_into()?;
        child.parents.push(base);
    }

    child.parents.push(env.clone());

    if let Some(expression) = source.expressions.pop() {
        return expression.eval(&child);
    }

    Ok(Type::Map(child))
}

impl Evaluable for token::Source {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        eval_source(self, env)
    }
}

impl Evaluable for token::Expression {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        use token::Expression::*;
        match self {
            Comparison(c) => c.eval(env),
            Function(arg_names, expression) => Ok(Type::Function(
                env.clone(),
                arg_names,
                Unevaluated::Expression(*expression),
            )),
            If(cond, cons, alt) => {
                let v = cond.eval(env)?;
                match v {
                    Type::Boolean(true) => cons.eval(env),
                    Type::Boolean(false) => alt.eval(env),
                    _ => Err(format_err!(
                        "conditional expression was evaluated to {}, not bool",
                        v
                    )),
                }
            }
        }
    }
}

impl Evaluable for token::Comparison {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        let mut base = self.left.eval(env)?;

        for right in self.rights {
            use token::ComparisonKind::*;
            let value = right.value.eval(env)?;
            match right.kind {
                Equal => base = Type::Boolean(base == value),
                NotEqual => base = Type::Boolean(base != value),
            }
        }
        Ok(base)
    }
}

impl Evaluable for token::Additive {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        let left = self.left.eval(env)?;

        if self.rights.is_empty() {
            return Ok(left);
        }

        let mut base: f64 = left.try_into()?;

        for right in self.rights {
            use token::AdditiveKind::*;
            let value: f64 = right.value.eval(env)?.try_into()?;

            match right.kind {
                Add => base += value,
                Sub => base -= value,
            };
        }

        return Ok(Type::Number(base));
    }
}

impl Evaluable for token::Multitive {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        let left = self.left.clone().eval(env)?;

        if self.rights.is_empty() {
            return Ok(left);
        }

        let mut base: f64 = left.try_into()?;

        for right in self.rights {
            use token::MultitiveKind::*;

            let value: f64 = right.value.eval(env)?.try_into()?;

            match right.kind {
                Mul => base *= value,
                Div => base /= value,
                Surplus => base %= value,
            }
        }
        return Ok(Type::Number(base));
    }
}

impl Evaluable for token::Primary {
    fn eval(mut self, env: &Env) -> Result<Type, failure::Error> {
        let mut base = self.0.remove(0).eval(env)?;

        for right in self.0 {
            if let token::Atom::Indentify(accessor) = right.base {
                base = base.get_prop(&accessor)?;

                for right in right.rights {
                    use token::PrimaryPartRight::*;
                    match right {
                        Indexing(arg) => {
                            let v = arg.eval(env)?;
                            match v {
                                Type::String(s) => base = base.get_prop(&s)?,
                                Type::Number(n) => base = base.indexing(n as i32)?,
                                _ => Err(format_err!("{} is not indexable", v))?,
                            }
                        }
                        Calling(expressions) => {
                            let args: Result<Vec<_>, _> =
                                expressions.into_iter().map(|e| e.eval(env)).collect();
                            base = base.call(args?)?;
                        }
                    }
                }
                continue;
            }
            panic!();
        }
        Ok(base)
    }
}

impl Evaluable for token::PrimaryPart {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        let mut base = self.base.eval(env)?;

        for right in self.rights {
            use token::PrimaryPartRight::*;
            match right {
                Indexing(arg) => {
                    let v = arg.eval(env)?;
                    match v {
                        Type::String(s) => base = base.get_prop(&s)?,
                        Type::Number(n) => base = base.indexing(n as i32)?,
                        _ => Err(format_err!("{} is not indexable", v))?,
                    }
                }
                Calling(expressions) => {
                    let args: Result<Vec<_>, _> =
                        expressions.into_iter().map(|e| e.eval(env)).collect();
                    base = base.call(args?)?;
                }
            }
        }
        Ok(base)
    }
}

impl Evaluable for token::Atom {
    fn eval(self, env: &Env) -> Result<Type, failure::Error> {
        use token::Atom::*;
        Ok(match self {
            Number(f) => Type::Number(f),
            String(s) => Type::String(s),
            Parenthesis(a) => a.eval(env)?,
            Block(s) => s.eval(env)?,
            Null => Type::Null,
            Indentify(s) => env.get_value(&s)?,
            List(v) => {
                let members: Result<Vec<_>, _> = v.into_iter().map(|e| e.eval(env)).collect();
                Type::List(members?)
            }
        })
    }
}
