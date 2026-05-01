use crate::lexer::Span;

pub type Spanned<T> = (T, Span);

#[derive(Clone, Debug)]
pub struct Statement {
    pub definitions: Vec<Bind>,
    pub body: Spanned<Expr>,
}

pub type Bind = (Spanned<String>, Spanned<Expr>);

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
}

#[derive(Clone, Copy, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    String(String),
    Variable(String),
    Null,
    List(Vec<Spanned<Expr>>),
    Function(Vec<Spanned<String>>, Box<Spanned<Expr>>),
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
    Access(Box<Spanned<Expr>>, Spanned<String>),
    Index(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
}

