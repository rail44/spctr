use crate::token::*;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum Cmd {
    Add,
    Sub,
    Div,
    Mul,
    Equal,
    NotEqual,
    Return,
    Load(usize, usize),
    NumberConst(f64),
    StringConst(Rc<String>),
    FunctionAddr,
    StructAddr(Rc<HashMap<String, Identifier>>),
    Label(usize),
    LabelAddr(usize),
    JumpRel(usize),
    JumpRelIf(usize),
    ProgramCounter,
    Store,
    Call,
}

#[derive(Clone, Debug)]
pub enum Identifier {
    Bind(usize),
    Arg(usize),
}

pub fn get_cmd(ast: &AST) -> Vec<Cmd> {
    let mut translator = Translator::new();
    translator.translate(ast)
}

struct Translator<'a> {
    env: HashMap<String, Identifier>,
    bind_cnt: usize,
    parent: Option<&'a Translator<'a>>,
}

impl<'a> Translator<'a> {
    fn new() -> Translator<'a> {
        Translator {
            env: HashMap::new(),
            bind_cnt: 0,
            parent: None,
        }
    }

    fn fork(&'a self) -> Translator<'a> {
        Translator {
            env: HashMap::new(),
            bind_cnt: self.bind_cnt,
            parent: Some(self),
        }
    }

    fn get_bind(&self, name: &str) -> Option<(Identifier, usize)> {
        self.env.get(name).map_or_else(
            || {
                self.parent
                    .and_then(|p| p.get_bind(name).map(|(addr, depth)| (addr, depth + 1)))
            },
            |addr| Some((addr.clone(), 0)),
        )
    }

    fn translate(&mut self, v: &Statement) -> Vec<Cmd> {
        let mut binds = Vec::new();
        for bind in v.definitions.iter() {
            let id = self.bind_cnt;
            self.env.insert(bind.0.clone(), Identifier::Bind(id));

            self.bind_cnt += 1;
            binds.push((id, &bind.1));
        }

        let mut cmd = Vec::new();
        for (id, body) in binds {
            let mut body_cmd = self.translate_expression(&body);
            body_cmd.push(Cmd::Return);

            cmd.push(Cmd::ProgramCounter);
            cmd.push(Cmd::NumberConst(5_f64));
            cmd.push(Cmd::Add);
            cmd.push(Cmd::Label(id));
            cmd.push(Cmd::JumpRel(body_cmd.len() + 1));
            cmd.append(&mut body_cmd);
        }

        cmd.append(&mut self.translate_expression(&v.body));
        cmd
    }

    fn translate_expression(&mut self, v: &Expression) -> Vec<Cmd> {
        match v {
            Expression::Comparison(a) => self.translate_comparison(a),
            Expression::If { cond, cons, alt } => {
                let mut cond_cmd = self.translate_expression(cond);

                let mut cons_cmd = self.translate_expression(cons);

                let mut alt_cmd = self.translate_expression(alt);
                alt_cmd.push(Cmd::JumpRel(cons_cmd.len() + 1));

                let mut cmd = Vec::new();

                cmd.append(&mut cond_cmd);
                cmd.push(Cmd::JumpRelIf(alt_cmd.len() + 1));

                cmd.append(&mut alt_cmd);
                cmd.append(&mut cons_cmd);

                cmd
            }
        }
    }

    fn translate_comparison(&mut self, v: &Comparison) -> Vec<Cmd> {
        let mut cmd = self.translate_additive(&v.left);
        for right in &v.rights {
            match right {
                ComparisonRight::Equal(r) => {
                    cmd.append(&mut self.translate_additive(&r));
                    cmd.push(Cmd::Equal);
                }
                ComparisonRight::NotEqual(r) => {
                    cmd.append(&mut self.translate_additive(&r));
                    cmd.push(Cmd::NotEqual);
                }
            }
        }
        cmd
    }

    fn translate_additive(&mut self, v: &Additive) -> Vec<Cmd> {
        let mut cmd = self.translate_multitive(&v.left);
        for right in &v.rights {
            match right {
                AdditiveRight::Add(r) => {
                    cmd.append(&mut self.translate_multitive(&r));
                    cmd.push(Cmd::Add);
                }
                AdditiveRight::Sub(r) => {
                    cmd.append(&mut self.translate_multitive(&r));
                    cmd.push(Cmd::Sub);
                }
            }
        }
        cmd
    }

    fn translate_multitive(&mut self, v: &Multitive) -> Vec<Cmd> {
        let mut cmd = self.translate_access(&v.left);
        for right in &v.rights {
            match right {
                MultitiveRight::Mul(r) => {
                    cmd.append(&mut self.translate_access(&r));
                    cmd.push(Cmd::Mul);
                }
                MultitiveRight::Div(r) => {
                    cmd.append(&mut self.translate_access(&r));
                    cmd.push(Cmd::Div);
                }
            }
        }
        cmd
    }

    fn translate_access(&mut self, v: &Access) -> Vec<Cmd> {
        let mut cmd = self.translate_primary(&v.left);
        cmd
    }

    fn translate_primary(&mut self, v: &Primary) -> Vec<Cmd> {
        match v {
            Primary::Number(v) => vec![Cmd::NumberConst(*v)],
            Primary::String(s) => vec![Cmd::StringConst(Rc::new(s.clone()))],
            Primary::Identifier(name) => self.translate_identifier(name),
            Primary::Block(statement) => {
                let mut translator = self.fork();
                translator.translate(statement)
            }
            Primary::Function(args, body) => {
                let mut translator = self.fork();
                let mut body_cmd = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    translator.env.insert(arg.clone(), Identifier::Arg(i));
                    body_cmd.push(Cmd::Store);
                }

                body_cmd.append(&mut translator.translate_expression(body));
                body_cmd.push(Cmd::Return);

                let mut cmd = Vec::new();
                cmd.push(Cmd::ProgramCounter);
                cmd.push(Cmd::NumberConst(5_f64));
                cmd.push(Cmd::Add);
                cmd.push(Cmd::FunctionAddr);
                cmd.push(Cmd::JumpRel(body_cmd.len() + 1));
                cmd.append(&mut body_cmd);
                cmd
            }
            Primary::Call(name, args) => {
                let mut cmd = Vec::new();
                for arg in args {
                    cmd.append(&mut self.translate_expression(arg));
                }
                let mut identifier_cmd = self.translate_identifier(name);
                cmd.append(&mut identifier_cmd);
                cmd.push(Cmd::Call);
                cmd
            }
            Primary::Struct(definitions) => {
                let mut translator = self.fork();
                let mut binds = Vec::new();
                for bind in definitions {
                    let id = translator.bind_cnt;
                    translator.env.insert(bind.0.clone(), Identifier::Bind(id));

                    translator.bind_cnt += 1;
                    binds.push((id, &bind.1));
                }

                let mut def_cmd = Vec::new();
                for (id, body) in binds {
                    let mut body_cmd = translator.translate_expression(&body);

                    def_cmd.push(Cmd::ProgramCounter);
                    def_cmd.push(Cmd::NumberConst(5_f64));
                    def_cmd.push(Cmd::Add);
                    def_cmd.push(Cmd::Label(id));
                    def_cmd.push(Cmd::JumpRel(body_cmd.len() + 1));
                    def_cmd.append(&mut body_cmd);
                }

                let mut cmd = Vec::new();
                cmd.push(Cmd::ProgramCounter);
                cmd.push(Cmd::NumberConst(5_f64));
                cmd.push(Cmd::Add);
                cmd.push(Cmd::StructAddr(Rc::new(translator.env)));
                cmd.push(Cmd::JumpRel(def_cmd.len() + 1));
                cmd.append(&mut def_cmd);

                cmd
            }
        }
    }

    fn translate_identifier(&self, name: &str) -> Vec<Cmd> {
        let id = self.get_bind(name).unwrap();
        let mut cmd = Vec::new();

        match id {
            (Identifier::Bind(id), _) => {
                cmd.push(Cmd::LabelAddr(id));
                cmd.push(Cmd::Call);
            }
            (Identifier::Arg(id), depth) => {
                cmd.push(Cmd::Load(id, depth));
            }
        };
        cmd
    }
}