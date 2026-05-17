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

    /// Plain string literal `"..."` (no interpolation). The interp-aware
    /// top-level `lex` function emits this for strings that contain no
    /// `${...}` regions; otherwise it emits the `StrBegin/StrLit/InterpOpen/
    /// InterpClose/StrEnd` sequence instead.
    Str(String),

    /// Opening `"` of an interpolated string.
    StrBegin,
    /// A literal text fragment between quotes / between an `}` and the next
    /// `${` or closing `"`. May be empty.
    StrLit(String),
    /// `${` — start of an interpolated expression.
    InterpOpen,
    /// `}` that closes an interpolated expression.
    InterpClose,
    /// Closing `"` of an interpolated string.
    StrEnd,

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
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
            Token::StrBegin => write!(f, "\""),
            Token::StrLit(s) => write!(f, "{}", s),
            Token::InterpOpen => write!(f, "${{"),
            Token::InterpClose => write!(f, "}}"),
            Token::StrEnd => write!(f, "\""),
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

/// Lexes `src` into a flat token stream.
///
/// String literals are scanned manually so we can recognize `${...}` interp
/// regions: each interp causes the lexer to flush a `StrLit` fragment, emit
/// `InterpOpen`, recursively lex the expression inside the braces (with
/// brace-depth tracking so the matching `}` becomes the interp close), and
/// then resume string mode. Strings with no `${` collapse to the legacy
/// single `Token::Str(s)` so existing call sites (e.g. the bracket-string
/// field access `r["foo"]`) keep their fast path.
pub fn lex(src: &str) -> Result<Vec<(Token, Span)>, Vec<LexError>> {
    let mut state = LexState {
        src,
        pos: 0,
        tokens: Vec::new(),
        errors: Vec::new(),
    };
    state.lex_top(false);
    if state.errors.is_empty() {
        Ok(state.tokens)
    } else {
        Err(state.errors)
    }
}

struct LexState<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<(Token, Span)>,
    errors: Vec<LexError>,
}

impl<'a> LexState<'a> {
    /// Lex tokens until end-of-input (when `inside_interp = false`) or
    /// until an unmatched `}` at depth zero (when `inside_interp = true`);
    /// in the interp case the closing `}` is consumed and emitted as
    /// `Token::InterpClose`, then this returns.
    fn lex_top(&mut self, inside_interp: bool) {
        let mut depth: i32 = 0;
        while self.pos < self.src.len() {
            let remaining = &self.src[self.pos..];
            let c = remaining.chars().next().unwrap();

            if c.is_whitespace() {
                self.pos += c.len_utf8();
                continue;
            }
            if remaining.starts_with("//") {
                let nl = remaining.find('\n').map(|n| n + 1).unwrap_or(remaining.len());
                self.pos += nl;
                continue;
            }
            if remaining.starts_with("/*") {
                if let Some(end) = remaining[2..].find("*/") {
                    self.pos += end + 4;
                } else {
                    self.errors.push(LexError {
                        span: self.pos..self.src.len(),
                    });
                    self.pos = self.src.len();
                }
                continue;
            }

            if c == '"' {
                self.lex_string();
                continue;
            }

            // Brace depth tracking — when inside an interp, the matching
            // `}` closes us out.
            if inside_interp && c == '}' && depth == 0 {
                self.tokens
                    .push((Token::InterpClose, self.pos..self.pos + 1));
                self.pos += 1;
                return;
            }

            // Defer to logos for one token at a time. Construct a fresh
            // Lexer for each iteration; this is cheap enough for our
            // inputs (REPL lines, ≤ 100KB programs).
            let mut sub_lex = Token::lexer(remaining);
            match sub_lex.next() {
                Some(Ok(t)) => {
                    let s = sub_lex.span();
                    if matches!(t, Token::LBrace | Token::LParen | Token::LBracket) {
                        depth += 1;
                    } else if matches!(t, Token::RBrace | Token::RParen | Token::RBracket) {
                        depth -= 1;
                    }
                    let abs = (s.start + self.pos)..(s.end + self.pos);
                    self.tokens.push((t, abs));
                    self.pos += s.end;
                }
                Some(Err(_)) => {
                    let s = sub_lex.span();
                    let abs = (s.start + self.pos)..(s.end + self.pos);
                    self.errors.push(LexError { span: abs });
                    self.pos += s.end.max(1);
                }
                None => break,
            }
        }
    }

    fn lex_string(&mut self) {
        let start = self.pos;
        self.pos += 1; // past opening "

        let mut current_lit = String::new();
        let mut lit_start = self.pos;
        // Tokens for an interp-shaped string are buffered locally so that
        // a plain-string fast path can collapse them into a single
        // `Token::Str(s)` only if no `${` was encountered.
        let mut buf: Vec<(Token, Span)> = Vec::new();
        let mut has_interp = false;

        loop {
            let Some(c) = self.src[self.pos..].chars().next() else {
                self.errors.push(LexError {
                    span: start..self.src.len(),
                });
                return;
            };

            if c == '"' {
                if has_interp {
                    buf.push((
                        Token::StrLit(std::mem::take(&mut current_lit)),
                        lit_start..self.pos,
                    ));
                    buf.push((Token::StrEnd, self.pos..self.pos + 1));
                    self.tokens.push((Token::StrBegin, start..start + 1));
                    self.tokens.extend(buf);
                } else {
                    self.tokens
                        .push((Token::Str(current_lit), start..self.pos + 1));
                }
                self.pos += 1;
                return;
            }

            if c == '\\' {
                let escape_start = self.pos;
                if self.pos + 1 >= self.src.len() {
                    self.errors.push(LexError {
                        span: escape_start..self.src.len(),
                    });
                    return;
                }
                let Some(next_c) = self.src[self.pos + 1..].chars().next() else {
                    self.errors.push(LexError {
                        span: escape_start..self.src.len(),
                    });
                    return;
                };
                let escaped: char = match next_c {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '"' => '"',
                    '/' => '/',
                    'b' => '\u{0008}',
                    'f' => '\u{000C}',
                    '$' => '$',
                    'u' => {
                        let hex_start = escape_start + 2;
                        if hex_start + 4 > self.src.len() {
                            self.errors.push(LexError {
                                span: escape_start..self.src.len(),
                            });
                            return;
                        }
                        let hex = &self.src[hex_start..hex_start + 4];
                        let Ok(code) = u32::from_str_radix(hex, 16) else {
                            self.errors.push(LexError {
                                span: escape_start..hex_start + 4,
                            });
                            return;
                        };
                        let Some(ch) = char::from_u32(code) else {
                            self.errors.push(LexError {
                                span: escape_start..hex_start + 4,
                            });
                            return;
                        };
                        current_lit.push(ch);
                        self.pos = hex_start + 4;
                        continue;
                    }
                    _ => {
                        self.errors.push(LexError {
                            span: escape_start..escape_start + 2,
                        });
                        return;
                    }
                };
                current_lit.push(escaped);
                self.pos += 1 + next_c.len_utf8();
                continue;
            }

            if c == '$' && self.src[self.pos + 1..].starts_with('{') {
                let interp_start = self.pos;
                has_interp = true;
                buf.push((
                    Token::StrLit(std::mem::take(&mut current_lit)),
                    lit_start..interp_start,
                ));
                buf.push((Token::InterpOpen, interp_start..interp_start + 2));
                self.pos = interp_start + 2;

                // Recursively lex the inner expression. The inner call
                // pushes tokens (and a final InterpClose) into self.tokens;
                // we swap them out so they accumulate into `buf` instead.
                let saved = std::mem::take(&mut self.tokens);
                self.lex_top(true);
                let inner = std::mem::replace(&mut self.tokens, saved);
                buf.extend(inner);

                lit_start = self.pos;
                continue;
            }

            current_lit.push(c);
            self.pos += c.len_utf8();
        }
    }
}
