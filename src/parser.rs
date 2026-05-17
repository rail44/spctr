use chumsky::input::{Stream, ValueInput};
use chumsky::prelude::*;

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::{lex, Token};
use crate::symbol::{intern, Symbol};

pub fn parse(src: &str) -> Result<Statement, Vec<Diagnostic>> {
    let tokens = lex(src).map_err(|errs| {
        errs.into_iter()
            .map(|e| Diagnostic::new(e.span, "lex error", "unexpected character"))
            .collect::<Vec<_>>()
    })?;

    let eoi: SimpleSpan = (src.len()..src.len()).into();
    let stream = Stream::from_iter(
        tokens
            .into_iter()
            .map(|(t, s)| (t, SimpleSpan::from(s.start..s.end))),
    )
    .map(eoi, |(t, s)| (t, s));

    parser().parse(stream).into_result().map_err(|errs| {
        errs.into_iter()
            .map(|e| {
                let span = span_to_range(*e.span());
                Diagnostic::new(span, "parse error", e.to_string())
            })
            .collect()
    })
}

fn span_to_range(s: SimpleSpan) -> std::ops::Range<usize> {
    s.start..s.end
}

fn parser<'src, I>(
) -> impl Parser<'src, I, Statement, extra::Err<Rich<'src, Token, SimpleSpan>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = SimpleSpan>,
{
    let ident = select! { Token::Ident(s) => intern(&s) }
        .map_with(|sym, ex| (sym, span_to_range(ex.span())));

    let key = select! {
        Token::Ident(s) => intern(&s),
        Token::Str(s) => intern(&s),
    }
    .map_with(|sym, ex| (sym, span_to_range(ex.span())));

    let expr = recursive(|expr| {
        let bind = key
            .clone()
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .map(|(name, value)| (name, value));

        let literal = select! {
            Token::Num(n) => Expr::Number(n),
            Token::Str(s) => Expr::String(std::rc::Rc::new(s)),
            Token::Null => Expr::Null,
            Token::True => Expr::Bool(true),
            Token::False => Expr::Bool(false),
        };

        let var = select! { Token::Ident(s) => Expr::Variable(VarRef::new(intern(&s))) };

        // Interpolated string: `StrBegin StrLit (InterpOpen expr InterpClose StrLit)* StrEnd`.
        // Plain strings flow through `literal` above (their `Token::Str` is
        // emitted directly by the lexer when no `${` was found).
        let interp_lit = select! { Token::StrLit(s) => s }
            .map_with(|s, ex| (s, span_to_range(ex.span())));
        let interp_string = just(Token::StrBegin)
            .ignore_then(interp_lit.clone())
            .then(
                just(Token::InterpOpen)
                    .ignore_then(expr.clone())
                    .then_ignore(just(Token::InterpClose))
                    .then(interp_lit.clone())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::StrEnd))
            .map(|((first, first_span), rest): ((String, std::ops::Range<usize>), Vec<(Spanned<Expr>, (String, std::ops::Range<usize>))>)| {
                let mut parts: Vec<InterpPart> = Vec::with_capacity(1 + rest.len() * 2);
                parts.push(InterpPart::Literal(std::rc::Rc::new(first), first_span));
                for (e, (lit, span)) in rest {
                    parts.push(InterpPart::Expr(e));
                    parts.push(InterpPart::Literal(std::rc::Rc::new(lit), span));
                }
                Expr::Interpolation(parts)
            });

        let list = expr
            .clone()
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(Expr::List);

        let func = ident
            .clone()
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .then_ignore(just(Token::FatArrow))
            .then(expr.clone())
            .map(|(args, body)| Expr::Function(args, Box::new(body)));

        #[derive(Clone)]
        enum BlockItem {
            Bind(Bind),
            Body(Spanned<Expr>),
        }

        let block_item = choice((
            key.clone()
                .then_ignore(just(Token::Colon))
                .rewind()
                .ignore_then(bind.clone())
                .map(BlockItem::Bind),
            expr.clone().map(BlockItem::Body),
        ));

        let brace = block_item
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .map(|items: Vec<BlockItem>| {
                let mut defs = Vec::new();
                let mut body = None;
                for item in items {
                    match item {
                        BlockItem::Bind(b) => defs.push(b),
                        BlockItem::Body(e) => body = Some(e),
                    }
                }
                match body {
                    Some(body) => Expr::ImmediateBlock(Box::new(Statement {
                        definitions: defs,
                        body,
                    })),
                    None => Expr::Block(defs),
                }
            });

        let if_expr = just(Token::If)
            .ignore_then(expr.clone())
            .then_ignore(just(Token::Then))
            .then(expr.clone())
            .then_ignore(just(Token::Else))
            .then(expr.clone())
            .map(|((cond, cons), alt)| Expr::If {
                cond: Box::new(cond),
                cons: Box::new(cons),
                alt: Box::new(alt),
            });

        let paren = expr
            .clone()
            .delimited_by(just(Token::LParen), just(Token::RParen));

        let atom = choice((literal, interp_string, if_expr, func, list, brace, var))
            .map_with(|e, ex| (e, span_to_range(ex.span())))
            .or(paren);

        #[derive(Clone)]
        enum PostfixOp {
            Access(Spanned<Symbol>),
            Call(Vec<Spanned<Expr>>),
            Index(Spanned<Expr>),
        }

        let dot_access = just(Token::Dot)
            .ignore_then(ident.clone())
            .map(PostfixOp::Access);

        let bracket_string_access = select! { Token::Str(s) => intern(&s) }
            .map_with(|sym, ex| (sym, span_to_range(ex.span())))
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(PostfixOp::Access);

        let bracket_index = expr
            .clone()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(PostfixOp::Index);

        let call = expr
            .clone()
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .map(PostfixOp::Call);

        let postfix_op = choice((dot_access, call, bracket_string_access, bracket_index));

        let postfix = atom
            .foldl_with(postfix_op.repeated(), |lhs, op, ex| {
                let span = span_to_range(ex.span());
                let node = match op {
                    PostfixOp::Access(name) => Expr::Access(Box::new(lhs), name),
                    PostfixOp::Call(args) => Expr::Call(Box::new(lhs), args),
                    PostfixOp::Index(idx) => Expr::Index(Box::new(lhs), Box::new(idx)),
                };
                (node, span)
            })
            .boxed();

        let unary = choice((
            just(Token::Minus).to(UnaryOp::Neg),
            just(Token::Bang).to(UnaryOp::Not),
        ))
        .repeated()
        .foldr_with(postfix, |op, rhs, ex| {
            (Expr::Unary(op, Box::new(rhs)), span_to_range(ex.span()))
        })
        .boxed();

        let mul_op = choice((
            just(Token::Star).to(BinOp::Mul),
            just(Token::Slash).to(BinOp::Div),
            just(Token::Percent).to(BinOp::Mod),
        ));

        let multitive = unary
            .clone()
            .foldl_with(mul_op.then(unary).repeated(), |lhs, (op, rhs), ex| {
                (
                    Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span_to_range(ex.span()),
                )
            })
            .boxed();

        let add_op = choice((
            just(Token::Plus).to(BinOp::Add),
            just(Token::Minus).to(BinOp::Sub),
        ));

        let additive = multitive
            .clone()
            .foldl_with(add_op.then(multitive).repeated(), |lhs, (op, rhs), ex| {
                (
                    Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span_to_range(ex.span()),
                )
            })
            .boxed();

        let cmp_op = choice((
            just(Token::NotEq).to(BinOp::Ne),
            just(Token::GtEq).to(BinOp::Ge),
            just(Token::LtEq).to(BinOp::Le),
            just(Token::EqEq).to(BinOp::Eq),
            just(Token::Gt).to(BinOp::Gt),
            just(Token::Lt).to(BinOp::Lt),
        ));

        let comparison = additive
            .clone()
            .foldl_with(cmp_op.then(additive).repeated(), |lhs, (op, rhs), ex| {
                (
                    Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span_to_range(ex.span()),
                )
            })
            .boxed();

        let logic_and = comparison
            .clone()
            .foldl_with(
                just(Token::AndAnd).to(BinOp::And).then(comparison).repeated(),
                |lhs, (op, rhs), ex| {
                    (
                        Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                        span_to_range(ex.span()),
                    )
                },
            )
            .boxed();

        logic_and
            .clone()
            .foldl_with(
                just(Token::OrOr).to(BinOp::Or).then(logic_and).repeated(),
                |lhs, (op, rhs), ex| {
                    (
                        Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                        span_to_range(ex.span()),
                    )
                },
            )
            .boxed()
    });

    let bind = key
        .clone()
        .then_ignore(just(Token::Colon))
        .then(expr.clone());

    #[derive(Clone)]
    enum StmtItem {
        Bind(Bind),
        Body(Spanned<Expr>),
    }

    let stmt_item = choice((
        key.clone()
            .then_ignore(just(Token::Colon))
            .rewind()
            .ignore_then(bind)
            .map(StmtItem::Bind),
        expr.clone().map(StmtItem::Body),
    ));

    stmt_item
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .then_ignore(end())
        .try_map(|items: Vec<StmtItem>, span| {
            let mut definitions = Vec::new();
            let mut body = None;
            for item in items {
                match item {
                    StmtItem::Bind(b) => definitions.push(b),
                    StmtItem::Body(e) => {
                        if body.is_some() {
                            return Err(Rich::custom(span, "multiple body expressions"));
                        }
                        body = Some(e);
                    }
                }
            }
            let body = body.unwrap_or_else(|| (Expr::Null, span_to_range(span)));
            Ok(Statement { definitions, body })
        })
}
