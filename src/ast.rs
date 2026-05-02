use crate::lexer::Span;
use crate::symbol::Symbol;
use std::cell::Cell;
use std::rc::Rc;

pub type Spanned<T> = (T, Span);

#[derive(Clone, Debug)]
pub struct Statement {
    pub definitions: Vec<Bind>,
    pub body: Spanned<Expr>,
}

pub type Bind = (Spanned<Symbol>, Spanned<Expr>);

#[derive(Clone, Copy, Debug)]
pub struct BindRef {
    pub depth: u32,
    pub slot: u32,
}

#[derive(Clone, Debug)]
pub struct VarRef {
    pub name: Symbol,
    pub resolved: Cell<Option<BindRef>>,
}

impl VarRef {
    pub fn new(name: Symbol) -> Self {
        Self {
            name,
            resolved: Cell::new(None),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And,
    Or,
}

#[derive(Clone, Copy, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    String(Rc<String>),
    Variable(VarRef),
    Null,
    Bool(bool),
    List(Vec<Spanned<Expr>>),
    Function(Vec<Spanned<Symbol>>, Box<Spanned<Expr>>),
    Block(Vec<Bind>),
    ImmediateBlock(Box<Statement>),
    If {
        cond: Box<Spanned<Expr>>,
        cons: Box<Spanned<Expr>>,
        alt: Box<Spanned<Expr>>,
    },
    Binary(BinOp, Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Unary(UnaryOp, Box<Spanned<Expr>>),
    Call(Box<Spanned<Expr>>, Vec<Spanned<Expr>>),
    Access(Box<Spanned<Expr>>, Spanned<Symbol>),
    Index(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
}

pub type AST = Statement;
