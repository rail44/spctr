use crate::parser;
use crate::token::*;
use crate::vm::ForeignFunction;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum Cmd {
    Add,
    Sub,
    Div,
    Mul,
    Surplus,
    Equal,
    GreaterThan,
    LessThan,
    Not,
    Load(usize, usize),
    Block(Vec<usize>),
    NumberConst(f64),
    StringConst(Rc<String>),
    NullConst,
    ConstructList(usize),
    ConstructFunction(usize),
    ConstructBlock(Rc<HashMap<String, usize>>),
    ForeignFunction(ForeignFunction),
    JumpRel(usize),
    JumpRelIf(usize),
    Call(usize),
    Index,
    Access,
    ExitScope,
}

pub fn get_cmd(ast: &AST) -> Vec<Cmd> {
    let mut translator = Translator::new();
    let mut cmd = Vec::new();

    let stdlib_names = vec!["Iterator"];
    for name in stdlib_names {
        let id = translator.bind_cnt;
        translator.env.insert(name.to_string(), id);
        translator.bind_cnt += 1;
    }

    let mut stdlib_cmds = vec![];

    let token = parser::parse(include_str!("iterator.spc")).unwrap().1;
    stdlib_cmds.push(translator.fork().translate(&token));

    let mut translator = translator.fork();
    let mut main_cmd = translator.translate(ast);
    cmd.push(Cmd::Block(
        stdlib_cmds.iter().map(|cmd| cmd.len()).collect(),
    ));
    cmd.append(&mut stdlib_cmds.into_iter().flatten().collect());
    cmd.append(&mut main_cmd);
    cmd.push(Cmd::ExitScope);
    cmd
}

struct Translator<'a> {
    env: HashMap<String, usize>,
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
            bind_cnt: 0,
            parent: Some(self),
        }
    }

    fn get_bind(&self, name: &str) -> Option<(usize, usize)> {
        self.env.get(name).map_or_else(
            || {
                self.parent
                    .and_then(|p| p.get_bind(name).map(|(addr, depth)| (addr, depth + 1)))
            },
            |addr| Some((*addr, 0)),
        )
    }

    fn translate(&mut self, v: &Statement) -> Vec<Cmd> {
        let mut cmd = Vec::new();

        let mut binds = Vec::new();
        for bind in v.definitions.iter() {
            let id = self.bind_cnt;
            self.env.insert(bind.0.clone(), id);

            self.bind_cnt += 1;
            binds.push(&bind.1)
        }

        let mut bind_cmds = Vec::new();
        for body in binds {
            bind_cmds.push(self.translate_expression(&body));
        }

        let mut body_cmd = self.translate_expression(&v.body);

        cmd.push(Cmd::Block(bind_cmds.iter().map(|cmd| cmd.len()).collect()));
        cmd.append(&mut bind_cmds.into_iter().flatten().collect());
        cmd.append(&mut body_cmd);
        cmd.push(Cmd::ExitScope);
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
                    cmd.push(Cmd::Equal);
                    cmd.push(Cmd::Not);
                }
                ComparisonRight::GreaterThan(r) => {
                    cmd.append(&mut self.translate_additive(&r));
                    cmd.push(Cmd::GreaterThan);
                }
                ComparisonRight::LessThan(r) => {
                    cmd.append(&mut self.translate_additive(&r));
                    cmd.push(Cmd::LessThan);
                }
                ComparisonRight::NotGreaterThan(r) => {
                    cmd.append(&mut self.translate_additive(&r));
                    cmd.push(Cmd::GreaterThan);
                    cmd.push(Cmd::Not);
                }
                ComparisonRight::NotLessThan(r) => {
                    cmd.append(&mut self.translate_additive(&r));
                    cmd.push(Cmd::LessThan);
                    cmd.push(Cmd::Not);
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
        let mut cmd = self.translate_operation(&v.left);
        for right in &v.rights {
            match right {
                MultitiveRight::Mul(r) => {
                    cmd.append(&mut self.translate_operation(&r));
                    cmd.push(Cmd::Mul);
                }
                MultitiveRight::Div(r) => {
                    cmd.append(&mut self.translate_operation(&r));
                    cmd.push(Cmd::Div);
                }
                MultitiveRight::Surplus(r) => {
                    cmd.append(&mut self.translate_operation(&r));
                    cmd.push(Cmd::Surplus);
                }
            }
        }
        cmd
    }

    fn translate_operation(&mut self, v: &Operation) -> Vec<Cmd> {
        let mut cmd = self.translate_primary(&v.left);
        for right in &v.rights {
            match right {
                OperationRight::Access(name) => {
                    cmd.push(Cmd::StringConst(Rc::new(name.clone())));
                    cmd.push(Cmd::Access);
                }
                OperationRight::Call(args) => {
                    for arg in args {
                        cmd.append(&mut self.translate_expression(arg));
                    }
                    cmd.push(Cmd::Call(args.len()));
                }
                OperationRight::Index(arg) => {
                    cmd.append(&mut self.translate_expression(arg));
                    cmd.push(Cmd::Index);
                }
            }
        }
        cmd
    }

    fn translate_primary(&mut self, v: &Primary) -> Vec<Cmd> {
        match v {
            Primary::Number(v) => vec![Cmd::NumberConst(*v)],
            Primary::Null => vec![Cmd::NullConst],
            Primary::String(s) => vec![Cmd::StringConst(Rc::new(s.clone()))],
            Primary::Variable(name) => self.translate_identifier(name),
            Primary::ImmediateBlock(statement) => {
                let mut translator = self.fork();
                translator.translate(statement)
            }
            Primary::Function(args, body) => {
                let mut translator = self.fork();
                let mut body_cmd = Vec::new();
                for arg in args {
                    translator.env.insert(arg.clone(), translator.bind_cnt);
                    translator.bind_cnt += 1;
                }

                body_cmd.append(&mut translator.translate_expression(body));

                let mut cmd = Vec::new();
                cmd.push(Cmd::ConstructFunction(body_cmd.len()));
                cmd.append(&mut body_cmd);
                cmd
            }
            Primary::Block(definitions) => {
                let mut binds = Vec::new();
                let mut translator = self.fork();
                for bind in definitions.iter() {
                    let id = translator.bind_cnt;
                    translator.env.insert(bind.0.clone(), id);

                    translator.bind_cnt += 1;
                    binds.push(&bind.1)
                }

                let mut bind_cmds = Vec::new();
                for body in binds {
                    bind_cmds.push(translator.translate_expression(&body));
                }

                let mut cmd = Vec::new();
                cmd.push(Cmd::Block(bind_cmds.iter().map(|cmd| cmd.len()).collect()));
                cmd.append(&mut bind_cmds.into_iter().flatten().collect());
                cmd.push(Cmd::ConstructBlock(Rc::new(translator.env)));
                cmd.push(Cmd::ExitScope);
                cmd
            }
            Primary::List(items) => {
                let mut cmd = Vec::new();
                for item in items {
                    cmd.append(&mut self.translate_expression(item));
                }

                cmd.push(Cmd::ConstructList(items.len()));
                cmd
            }
        }
    }

    fn translate_identifier(&self, name: &str) -> Vec<Cmd> {
        let (id, depth) = self
            .get_bind(name)
            .unwrap_or_else(|| panic!("could not find bind by \"{}\"", name));
        let mut cmd = Vec::new();
        cmd.push(Cmd::Load(id, depth));
        cmd
    }
}
