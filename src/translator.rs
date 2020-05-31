use crate::parser;
use crate::token::*;
use crate::vm::{Cmd, ForeignFunction, Value};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn get_cmd(ast: &AST) -> Vec<Cmd> {
    let mut translator = Translator::new();
    let mut block = translator.block();

    block.add_bind("Iterator", |translator| {
        let token = parser::parse(include_str!("iterator.spc")).unwrap().1;
        translator.fork().translate(&token)
    });

    block.add_bind("List", |translator| {
        let mut translator = translator.fork();
        let mut block = translator.block();

        block.add_bind("concat", |_| vec![
            Cmd::ForeignFunction(ForeignFunction(Rc::new(move |_, mut args| {
                let mut target = (*args.pop().unwrap().into_list().unwrap()).clone();
                let mut dst = (*args.pop().unwrap().into_list().unwrap()).clone();
                target.append(&mut dst);
                Value::list(Rc::new(target))
            })))
        ]);
        block.set_body(move |translator| {
            let mut cmd = Vec::new();
            let mut load_cmds = Vec::new();
            for i in 0..1 {
                load_cmds.push(Cmd::Load(i, 0));
                load_cmds.push(Cmd::Return);
            }
            cmd.push(Cmd::ConstructBlock(
                load_cmds.len(),
                Rc::new(translator.env.clone()),
            ));
            cmd.append(&mut load_cmds);
            cmd
        });
        block.finalize()
    });

    block.add_bind("String", |translator| {
        let mut translator = translator.fork();
        let mut block = translator.block();

        block.add_bind("concat", |_| vec![
            Cmd::ForeignFunction(ForeignFunction(Rc::new(move |_, mut args| {
                let target = args.pop().unwrap().into_string().unwrap();
                let dst = args.pop().unwrap().into_string().unwrap();
                Value::string(Rc::new(format!("{}{}", target, dst)))
            })))
        ]);
        block.set_body(move |translator| {
            let mut cmd = Vec::new();
            let mut load_cmds = Vec::new();
            for i in 0..1 {
                load_cmds.push(Cmd::Load(i, 0));
                load_cmds.push(Cmd::Return);
            }
            cmd.push(Cmd::ConstructBlock(
                load_cmds.len(),
                Rc::new(translator.env.clone()),
            ));
            cmd.append(&mut load_cmds);
            cmd
        });
        block.finalize()
    });
    block.set_body(|translator| translator.fork().translate(ast));
    block.finalize()
}

struct BlockTranslator<'a> {
    translator: &'a mut Translator<'a>,
    bind_names: Vec<String>,
    bind_bodies: Vec<Box<dyn FnOnce(&mut Translator) -> Vec<Cmd> + 'a>>,
    body: Option<Box<dyn FnOnce(&mut Translator) -> Vec<Cmd> + 'a>>,
}

impl<'a> BlockTranslator<'a> {
    fn add_bind<S, F>(&mut self, name: S, f: F)
    where
        S: ToString,
        F: FnOnce(&mut Translator) -> Vec<Cmd> + 'a,
    {
        self.bind_names.push(name.to_string());
        self.bind_bodies.push(Box::new(f));
    }

    fn set_body<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Translator) -> Vec<Cmd> + 'a,
    {
        self.body = Some(Box::new(f));
    }

    fn finalize(self) -> Vec<Cmd> {
        let mut cmd = Vec::new();
        let mut b = Vec::new();
        for name in self.bind_names {
            let id = self.translator.define_bind(name.to_string());
            b.push(id);
        }

        let mut bind_cmds = Vec::new();
        for (id, f) in b.into_iter().zip(self.bind_bodies) {
            let mut body_cmd = f(self.translator);
            body_cmd.push(Cmd::Store(id));
            body_cmd.push(Cmd::Return);
            bind_cmds.push(body_cmd);
        }

        let mut body_cmd = (self.body.unwrap())(self.translator);

        cmd.push(Cmd::Block(bind_cmds.iter().map(|cmd| cmd.len()).collect()));
        cmd.append(&mut bind_cmds.into_iter().flatten().collect());
        cmd.append(&mut body_cmd);
        cmd.push(Cmd::ExitScope);
        cmd
    }
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

    fn block(&'a mut self) -> BlockTranslator<'a> {
        BlockTranslator {
            translator: self,
            bind_bodies: Vec::new(),
            bind_names: Vec::new(),
            body: None,
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
        self.env.insert(name, id);

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

    fn translate(&'a mut self, v: &'a Statement) -> Vec<Cmd> {
        let mut block = self.block();
        for (name, body) in &v.definitions {
            block.add_bind(name, move |translator: &mut Translator| {
                translator.translate_expression(&body)
            });
        }
        block.set_body(move |translator| translator.translate_expression(&v.body));
        block.finalize()
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
                let mut translator = self.fork();
                let mut block = translator.block();

                for (name, body) in definitions.iter() {
                    block.add_bind(name, move |translator: &mut Translator| {
                        translator.translate_expression(body)
                    });
                }

                block.set_body(move |translator| {
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
                });
                block.finalize()
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
