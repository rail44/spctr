use std::rc::Rc;
use std::cell::RefCell;
use crate::{Env, Type, token, array, string};

pub fn eval_source (mut source: token::Source, env: Option<&mut Env>) -> Type {
    if let Some(expression) = source.expressions.pop() {
        let mut env = Env {
            binds: source.binds,
            evaluated: [
                ("Array".to_string(), array::Array.into())
            ].iter().cloned().collect(),
            parent: env.map(|e| Rc::new(RefCell::new(e.clone())))
        };
        return expression.eval(&mut env)
    }

    Type::Map(source.binds)
}

impl Evaluable for token::Source {
    fn eval(self, env: &mut Env) -> Type {
        eval_source(self, Some(env))
    }
}


pub trait Evaluable {
    fn eval(self, env: &mut Env) -> Type;
}

impl Evaluable for token::Expression {
    fn eval(self, env: &mut Env) -> Type {
        use token::Expression::*;
        match self {
            Comparison(c) => c.eval(env),
            Function(arg_names, expression) => Type::Function(env.clone(), arg_names, *expression),
        }
    }
}

impl Evaluable for token::Comparison {
    fn eval(self, env: &mut Env) -> Type {
        let mut base = self.left.eval(env);

        for right in self.rights {
            use token::ComparisonKind::*;
            let value = right.value.eval(env);
            match right.kind {
                Equal => base = Type::Boolean(base == value),
                NotEqual => base = Type::Boolean(base != value),
            }
        }
        base
    }
}

impl Evaluable for token::Additive {
    fn eval(self, env: &mut Env) -> Type {
        let left = self.left.eval(env);

        if self.rights.len() == 0 {
            return  left;
        }

        if let Type::Number(mut base) = left {
            for right in self.rights {
                use token::AdditiveKind::*;
                if let Type::Number(value) = right.value.eval(env) {
                    match right.kind {
                        Add => base += value,
                        Sub => base -= value,
                    }
                    continue;
                }
                panic!("not a number");
            }
            return Type::Number(base);
        }
        panic!("not a number");
    }
}

impl Evaluable for token::Multitive {
    fn eval(self, env: &mut Env) -> Type {
        let left = self.left.clone().eval(env);

        if self.rights.len() == 0 {
            return  left;
        }

        if let Type::Number(mut base) = left {
            for right in self.rights {
                if let Type::Number(value) = right.value.clone().eval(env) {
                    use token::MultitiveKind::*;
                    match right.kind {
                        Mul => base *= value,
                        Div => base /= value,
                        Surplus => base = base % value,
                    }
                    continue;
                }
                panic!("not a number: {:?}", right);
            }
            return Type::Number(base);
        }
        panic!("not a number: {:?}", self.left.clone());
    }
}

impl Evaluable for token::Evaluation {
    fn eval(self, env: &mut Env) -> Type {
        let mut base = env.get_value(&self.left);

        for right in self.rights {
            use token::EvaluationRight::*;
            match right {
                Access(name) => {
                    base = base.get_prop(env, &name);
                }
                Call(arg) => {
                    base = base.call(&mut env.clone(), vec![arg.eval(env)]);
                }
            }
        }
        base
    }
}

impl Evaluable for token::Primary {
    fn eval(self, env: &mut Env) -> Type {
        use token::Primary::*;
        match self {
            Number(f) => Type::Number(f),
            String(s) => Type::String(s),
            Parenthesis(a) => a.eval(env),
            Block(s) => s.eval(env),
            Evaluation(e) => e.eval(env),
            If(cond, cons, alt) => {
                match cond.eval(env) {
                    Type::Boolean(true) => cons.eval(env),
                    Type::Boolean(false) => alt.eval(env),
                    _ => panic!(),
                }
            }
        }
    }
}
