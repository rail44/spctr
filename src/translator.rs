use crate::parser;
use crate::token::*;
use crate::vm::{Cmd, ForeignFunction, Value};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn get_cmd(ast: &AST) -> Vec<Cmd> {
    let mut translator = Translator::new();
    let mut cmd = Vec::new();

    let stdlib_names = vec!["Iterator", "List", "String"];
    for name in stdlib_names {
        let id = translator.bind_cnt;
        translator.env.insert(name.to_string(), id);
        translator.bind_cnt += 1;
    }

    let mut stdlib_cmds = vec![];

    let token = parser::parse(include_str!("iterator.spc")).unwrap().1;
    let mut iterator_cmd = translator.fork().translate(&token);
    iterator_cmd.push(Cmd::Store(0));
    iterator_cmd.push(Cmd::Return);
    stdlib_cmds.push(iterator_cmd);

    let mut list_cmd = Vec::new();
    {
        let mut translator = translator.fork();
        let mut field_cmds = Vec::new();
        let mut load_cmds = Vec::new();

        let field_names = vec!["concat"];
        for name in field_names {
            let id = translator.bind_cnt;
            translator.env.insert(name.to_string(), id);
            load_cmds.push(Cmd::Load(id, 0));
            load_cmds.push(Cmd::Return);
            translator.bind_cnt += 1;
        }

        field_cmds.push(vec![
            Cmd::ForeignFunction(ForeignFunction(Rc::new(move |_, mut args| {
                let mut target = (*args.pop().unwrap().into_list().unwrap()).clone();
                let mut dst = (*args.pop().unwrap().into_list().unwrap()).clone();
                target.append(&mut dst);
                Value::list(Rc::new(target))
            }))),
            Cmd::Store(0),
            Cmd::Return,
        ]);
        list_cmd.push(Cmd::Block(field_cmds.iter().map(|cmd| cmd.len()).collect()));
        list_cmd.append(&mut field_cmds.into_iter().flatten().collect());
        list_cmd.push(Cmd::ConstructBlock(
            load_cmds.len(),
            Rc::new(translator.env),
        ));
        list_cmd.append(&mut load_cmds);
        list_cmd.push(Cmd::ExitScope);
        list_cmd.push(Cmd::Store(1));
        list_cmd.push(Cmd::Return);
    }
    stdlib_cmds.push(list_cmd);

    let mut string_cmd = Vec::new();
    {
        let mut translator = translator.fork();
        let mut field_cmds = Vec::new();
        let mut load_cmds = Vec::new();

        let field_names = vec!["concat"];
        for name in field_names {
            let id = translator.bind_cnt;
            translator.env.insert(name.to_string(), id);
            load_cmds.push(Cmd::Load(id, 0));
            load_cmds.push(Cmd::Return);
            translator.bind_cnt += 1;
        }

        field_cmds.push(vec![
            Cmd::ForeignFunction(ForeignFunction(Rc::new(move |_, mut args| {
                let target = args.pop().unwrap().into_string().unwrap();
                let dst = args.pop().unwrap().into_string().unwrap();
                Value::string(Rc::new(format!("{}{}", target, dst)))
            }))),
            Cmd::Store(0),
            Cmd::Return,
        ]);
        string_cmd.push(Cmd::Block(field_cmds.iter().map(|cmd| cmd.len()).collect()));
        string_cmd.append(&mut field_cmds.into_iter().flatten().collect());
        string_cmd.push(Cmd::ConstructBlock(
            load_cmds.len(),
            Rc::new(translator.env),
        ));
        string_cmd.append(&mut load_cmds);
        string_cmd.push(Cmd::ExitScope);
        string_cmd.push(Cmd::Store(2));
        string_cmd.push(Cmd::Return);
    }
    stdlib_cmds.push(string_cmd);

    cmd.push(Cmd::Block(
        stdlib_cmds.iter().map(|cmd| cmd.len()).collect(),
    ));
    cmd.append(&mut stdlib_cmds.into_iter().flatten().collect());

    let mut main_cmd = translator.fork().translate(ast);
    cmd.append(&mut main_cmd);
    cmd.push(Cmd::ExitScope);

    cmd
}

struct Translator<'a> {
    env: HashMap<String, usize>,
    bind_cnt: usize,
    parent: Option<&'a Translator<'a>>,
    function_id: Rc<Cell<usize>>,
}

impl<'a> Translator<'a> {
    fn new() -> Translator<'a> {
        Translator {
            env: HashMap::new(),
            bind_cnt: 0,
            parent: None,
            function_id: Rc::new(Cell::new(0)),
        }
    }

    fn fork(&'a self) -> Translator<'a> {
        Translator {
            env: HashMap::new(),
            bind_cnt: 0,
            parent: Some(self),
            function_id: self.function_id.clone(),
        }
    }

    fn define_bind(&mut self, name: String) -> usize {
        let id = self.bind_cnt;
        self.env.insert(name.clone(), id);

        self.bind_cnt += 1;
        id
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

    fn translate_block<S, F1, F2>(&mut self, binds: Vec<(S, F1)>, body_f: F2) -> Vec<Cmd>
    where
        S: ToString,
        F1: Fn(&mut Self) -> Vec<Cmd>,
        F2: Fn(&mut Self) -> Vec<Cmd>,
    {
        let mut cmd = Vec::new();
        let mut b = Vec::new();
        for (name, f) in binds.iter() {
            let id = self.define_bind(name.to_string());
            b.push((id, f));
        }

        let mut bind_cmds = Vec::new();
        for (id, f) in b {
            let mut body_cmd = f(self);
            body_cmd.push(Cmd::Store(id));
            body_cmd.push(Cmd::Return);
            bind_cmds.push(body_cmd);
        }

        let mut body_cmd = body_f(self);

        cmd.push(Cmd::Block(bind_cmds.iter().map(|cmd| cmd.len()).collect()));
        cmd.append(&mut bind_cmds.into_iter().flatten().collect());
        cmd.append(&mut body_cmd);
        cmd.push(Cmd::ExitScope);
        cmd
    }

    fn translate(&mut self, v: &Statement) -> Vec<Cmd> {
        self.translate_block(
            v.definitions
                .iter()
                .map(|(n, b)| (n, move |translator: &mut Translator| translator.translate_expression(&b)))
                .collect(),
            |translator| translator.translate_expression(&v.body),
        )
    }

    fn translate_expression(&mut self, v: &Expression) -> Vec<Cmd> {
        match v {
            Expression::Comparison(a) => self.translate_comparison(a),
            Expression::If { cond, cons, alt } => {
                let mut cond_cmd = self.translate_expression(cond);

                let mut alt_cmd = self.translate_expression(alt);

                let mut cons_cmd = self.translate_expression(cons);
                cons_cmd.push(Cmd::JumpRel(alt_cmd.len() + 1));

                let mut cmd = Vec::new();

                cmd.append(&mut cond_cmd);
                cmd.push(Cmd::JumpRelUnless(cons_cmd.len() + 1));

                cmd.append(&mut cons_cmd);
                cmd.append(&mut alt_cmd);

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
            Primary::Function(arg_names, body) => {
                let mut translator = self.fork();
                let mut body_cmd = Vec::new();
                for arg in arg_names {
                    translator.define_bind(arg.clone());
                }

                body_cmd.append(&mut translator.translate_expression(body));
                body_cmd.push(Cmd::ExitScope);
                body_cmd.push(Cmd::Return);

                let mut cmd = Vec::new();
                let id = self.function_id.get();
                cmd.push(Cmd::ConstructFunction(id, body_cmd.len()));
                self.function_id.set(id + 1);
                cmd.append(&mut body_cmd);
                cmd
            }
            Primary::Block(definitions) => {
                let l = definitions.len();
                self.fork().translate_block(
                    definitions
                        .iter()
                        .map(|(n, b)| (n, move |translator: &mut Translator| translator.translate_expression(&b)))
                        .collect(),
                    |translator| {
                        let mut cmd = Vec::new();
                        let mut load_cmds = Vec::new();
                        for i in 0..l {
                            load_cmds.push(Cmd::Load(i, 0));
                            load_cmds.push(Cmd::Return);
                        }
                        cmd.push(Cmd::ConstructBlock(
                            load_cmds.len(),
                            Rc::new(translator.env.clone()),
                        ));
                        cmd.append(&mut load_cmds);
                        cmd
                    },
                )
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
