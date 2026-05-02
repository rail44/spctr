use logos::Logos;
use std::ops::Range;

pub type Span = Range<usize>;

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
pub enum Token {
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("null")]
    Null,

    #[token("=>")]
    FatArrow,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    #[token("!=")]
    NotEq,
    #[token(">=")]
    GtEq,
    #[token("<=")]
    LtEq,
    #[token("=")]
    Eq,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("!")]
    Bang,

    #[regex(r"[0-9]+(\.[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Num(f64),

    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        s[1..s.len() - 1].to_string()
    })]
    Str(String),

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Null => write!(f, "null"),
            Token::FatArrow => write!(f, "=>"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Dot => write!(f, "."),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::NotEq => write!(f, "!="),
            Token::GtEq => write!(f, ">="),
            Token::LtEq => write!(f, "<="),
            Token::Eq => write!(f, "="),
            Token::Gt => write!(f, ">"),
            Token::Lt => write!(f, "<"),
            Token::Bang => write!(f, "!"),
            Token::Num(n) => write!(f, "{}", n),
            Token::Str(s) => write!(f, "\"{}\"", s),
            Token::Ident(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug)]
pub struct LexError {
    pub span: Span,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unexpected character at {}..{}", self.span.start, self.span.end)
    }
}

pub fn lex(src: &str) -> Result<Vec<(Token, Span)>, Vec<LexError>> {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    for (res, span) in Token::lexer(src).spanned() {
        match res {
            Ok(t) => tokens.push((t, span)),
            Err(_) => errors.push(LexError { span }),
        }
    }
    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}
