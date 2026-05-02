use logos::Logos;
use std::ops::Range;

pub type Span = Range<usize>;

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
// `[^\n]*` is intentionally greedy: a `//` comment runs to end-of-line.
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
pub enum Token {
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("null")]
    Null,
    #[token("true")]
    True,
    #[token("false")]
    False,

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

    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    GtEq,
    #[token("<=")]
    LtEq,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("!")]
    Bang,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,

    #[regex(r"[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Num(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let raw = lex.slice();
        unescape(&raw[1..raw.len()-1])
    })]
    Str(String),

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '/' => out.push('/'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000C}'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 {
                        return None;
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(code)?);
                }
                _ => return None,
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::If => write!(f, "if"),
            Token::Then => write!(f, "then"),
            Token::Else => write!(f, "else"),
            Token::Null => write!(f, "null"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
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
            Token::EqEq => write!(f, "=="),
            Token::NotEq => write!(f, "!="),
            Token::GtEq => write!(f, ">="),
            Token::LtEq => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::Lt => write!(f, "<"),
            Token::Bang => write!(f, "!"),
            Token::AndAnd => write!(f, "&&"),
            Token::OrOr => write!(f, "||"),
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
        write!(
            f,
            "unexpected character at {}..{}",
            self.span.start, self.span.end
        )
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
