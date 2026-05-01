use crate::ast::*;
use crate::parser;
use crate::stdlib;
use crate::vm::{Cmd, ForeignFunction, Value};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct Env(Option<Rc<(HashMap<String, usize>, Env)>>);

impl Env {
    fn push(&mut self, map: HashMap<String, usize>) {
        self.0 = Some(Rc::new((map, Env(self.0.take()))));
    }

    fn pop(&mut self) -> HashMap<String, usize> {
        let rc = self.0.take().unwrap();
        let (head, tail) = Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone());
        *self = tail;
        head
    }

    fn get_bind(&self, name: &str) -> Option<(usize, usize)> {
        let rc = self.0.as_ref().unwrap();
        rc.0.get(name).map_or_else(
            || rc.1.get_bind(name).map(|(addr, depth)| (addr, depth + 1)),
            |addr| Some((*addr, 0)),
        )
    }
}

pub fn get_cmd(ast: &AST) -> Vec<Cmd> {
    let mut translator = Translator::new();
    let mut block = translator.block();

    block.add_bind("Iterator", |translator| {
        let stmt = parser::parse(include_str!("stdlib/iterator.spc")).unwrap();
        translator.translate(&stmt)
    });

    block.add_bind("List", stdlib::list::get_module);
    block.add_bind("String", stdlib::string::get_module);

    block.set_body(|translator| translator.translate(ast));
    block.finalize()
}

pub struct BlockTranslator<'a> {
    translator: &'a mut Translator,
    bind_names: Vec<String>,
    bind_bodies: Vec<Box<dyn FnOnce(&mut Translator) -> Vec<Cmd> + 'a>>,
    body: Option<Box<dyn FnOnce(&mut Translator) -> Vec<Cmd> + 'a>>,
}

impl<'a> BlockTranslator<'a> {
    pub fn add_bind<S, F>(&mut self, name: S, f: F)
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

    pub fn finalize(self) -> Vec<Cmd> {
        let mut cmd = Vec::new();
        let mut b = Vec::new();
        let l = self.bind_names.len();
        let mut map = HashMap::new();
        for (id, name) in self.bind_names.into_iter().enumerate() {
            map.insert(name, id);
            b.push(id);
        }

        let mut translator = self.translator.fork(map);
        let mut bind_cmds = Vec::new();
        for (id, f) in b.into_iter().zip(self.bind_bodies) {
            let mut body_cmd = f(&mut translator);
            body_cmd.push(Cmd::Store(id));
            body_cmd.push(Cmd::Return);
            bind_cmds.push(body_cmd);
        }

        cmd.push(Cmd::Block(bind_cmds.iter().map(|cmd| cmd.len()).collect()));
        cmd.append(&mut bind_cmds.into_iter().flatten().collect());

        let mut body = if let Some(body_cmd) = self.body {
            (body_cmd)(&mut translator)
        } else {
            let mut cmd = Vec::new();
            let mut load_cmds = Vec::new();
            for i in 0..l {
                load_cmds.push(Cmd::Load(i, 0));
                load_cmds.push(Cmd::Return);
            }
            cmd.push(Cmd::ConstructBlock(
                load_cmds.len(),
                Rc::new(translator.env.pop()),
            ));
            cmd.append(&mut load_cmds);
            cmd
        };

        cmd.append(&mut body);
        cmd.push(Cmd::ExitScope);
        cmd
    }
}

pub struct Translator {
    env: Env,
}

impl Translator {
    fn new() -> Translator {
        Translator { env: Env(None) }
    }

    pub fn block(&mut self) -> BlockTranslator<'_> {
        BlockTranslator {
            translator: self,
            bind_bodies: Vec::new(),
            bind_names: Vec::new(),
            body: None,
        }
    }

    pub fn fork(&self, map: HashMap<String, usize>) -> Translator {
        let mut forked_env = self.env.clone();
        forked_env.push(map);
        Translator { env: forked_env }
    }

    fn get_bind(&self, name: &str) -> Option<(usize, usize)> {
        self.env.get_bind(name)
    }

    fn translate(&mut self, stmt: &Statement) -> Vec<Cmd> {
        let mut block = self.block();
        for ((name, _name_span), body) in &stmt.definitions {
            block.add_bind(name.clone(), move |translator: &mut Translator| {
                translator.translate_expr(body)
            });
        }
        let body = &stmt.body;
        block.set_body(move |translator| translator.translate_expr(body));
        block.finalize()
    }

    fn translate_expr(&mut self, expr: &Spanned<Expr>) -> Vec<Cmd> {
        match &expr.0 {
            Expr::Number(n) => vec![Cmd::NumberConst(*n)],
            Expr::String(s) => vec![Cmd::StringConst(Rc::new(s.clone()))],
            Expr::Null => vec![Cmd::NullConst],
            Expr::Variable(name) => self.translate_identifier(name),
            Expr::List(items) => {
                let mut cmd = Vec::new();
                for item in items {
                    cmd.append(&mut self.translate_expr(item));
                }
                cmd.push(Cmd::ConstructList(items.len()));
                cmd
            }
            Expr::Function(arg_names, body) => {
                let mut map = HashMap::new();
                for (id, (name, _)) in arg_names.iter().enumerate() {
                    map.insert(name.clone(), id);
                }
                let mut translator = self.fork(map);

                let mut body_cmd = translator.translate_expr(body);
                body_cmd.push(Cmd::Return);

                let mut cmd = Vec::new();
                cmd.push(Cmd::ConstructFunction(body_cmd.len()));
                cmd.append(&mut body_cmd);
                cmd
            }
            Expr::Block(definitions) => {
                let mut block = self.block();
                for ((name, _), body) in definitions.iter() {
                    block.add_bind(name.clone(), move |translator: &mut Translator| {
                        translator.translate_expr(body)
                    });
                }
                block.finalize()
            }
            Expr::ImmediateBlock(stmt) => self.translate(stmt),
            Expr::If { cond, cons, alt } => {
                let mut cond_cmd = self.translate_expr(cond);
                let mut alt_cmd = self.translate_expr(alt);
                let mut cons_cmd = self.translate_expr(cons);
                cons_cmd.push(Cmd::JumpRel(alt_cmd.len() + 1));

                let mut cmd = Vec::new();
                cmd.append(&mut cond_cmd);
                cmd.push(Cmd::JumpRelUnless(cons_cmd.len() + 1));
                cmd.append(&mut cons_cmd);
                cmd.append(&mut alt_cmd);
                cmd
            }
            Expr::Binary(op, l, r) => {
                let mut cmd = self.translate_expr(l);
                cmd.append(&mut self.translate_expr(r));
                match op {
                    BinOp::Add => cmd.push(Cmd::Add),
                    BinOp::Sub => cmd.push(Cmd::Sub),
                    BinOp::Mul => cmd.push(Cmd::Mul),
                    BinOp::Div => cmd.push(Cmd::Div),
                    BinOp::Mod => cmd.push(Cmd::Surplus),
                    BinOp::Eq => cmd.push(Cmd::Equal),
                    BinOp::Ne => {
                        cmd.push(Cmd::Equal);
                        cmd.push(Cmd::Not);
                    }
                    BinOp::Gt => cmd.push(Cmd::GreaterThan),
                    BinOp::Lt => cmd.push(Cmd::LessThan),
                    BinOp::Ge => {
                        cmd.push(Cmd::LessThan);
                        cmd.push(Cmd::Not);
                    }
                    BinOp::Le => {
                        cmd.push(Cmd::GreaterThan);
                        cmd.push(Cmd::Not);
                    }
                }
                cmd
            }
            Expr::Unary(op, e) => {
                let mut cmd = match op {
                    UnaryOp::Neg => vec![Cmd::NumberConst(0.0)],
                    UnaryOp::Not => Vec::new(),
                };
                cmd.append(&mut self.translate_expr(e));
                match op {
                    UnaryOp::Neg => cmd.push(Cmd::Sub),
                    UnaryOp::Not => cmd.push(Cmd::Not),
                }
                cmd
            }
            Expr::Call(callee, args) => {
                let mut cmd = self.translate_expr(callee);
                for arg in args {
                    cmd.append(&mut self.translate_expr(arg));
                }
                cmd.push(Cmd::Call(args.len()));
                cmd
            }
            Expr::Access(obj, (name, _)) => {
                let mut cmd = self.translate_expr(obj);
                cmd.push(Cmd::StringConst(Rc::new(name.clone())));
                cmd.push(Cmd::Access);
                cmd
            }
            Expr::Index(arr, idx) => {
                let mut cmd = self.translate_expr(arr);
                cmd.append(&mut self.translate_expr(idx));
                cmd.push(Cmd::Index);
                cmd
            }
        }
    }

    fn translate_identifier(&self, name: &str) -> Vec<Cmd> {
        let (id, depth) = self
            .get_bind(name)
            .unwrap_or_else(|| panic!("could not find bind by \"{}\"", name));
        vec![Cmd::Load(id, depth)]
    }

    pub fn translate_foreign<F>(&self, f: F) -> Vec<Cmd>
    where
        F: Fn(Vec<Value>) -> Value + 'static,
    {
        vec![Cmd::ConstructForeignFunction(ForeignFunction(Rc::new(f)))]
    }
}
