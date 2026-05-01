use chumsky::input::{Stream, ValueInput};
use chumsky::prelude::*;

use crate::ast::*;
use crate::lexer::{lex, Token};

pub fn parse(src: &str) -> Result<Statement, Vec<String>> {
    let tokens = lex(src).map_err(|errs| {
        errs.into_iter()
            .map(|e| format!("Lex error: {}", e))
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
            .map(|e| format!("Parse error at {}..{}: {:?}", e.span().start, e.span().end, e))
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
    let ident = select! { Token::Ident(s) => s }
        .map_with(|s, ex| (s, span_to_range(ex.span())));

    let expr = recursive(|expr| {
        let bind = ident
            .clone()
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .map(|(name, value)| (name, value));

        let literal = select! {
            Token::Num(n) => Expr::Number(n),
            Token::Str(s) => Expr::String(s),
            Token::Null => Expr::Null,
        };

        let var = select! { Token::Ident(s) => Expr::Variable(s) };

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
            ident
                .clone()
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

        let atom = choice((literal, if_expr, func, list, brace, var))
            .map_with(|e, ex| (e, span_to_range(ex.span())))
            .or(paren);

        #[derive(Clone)]
        enum PostfixOp {
            Access(Spanned<String>),
            Call(Vec<Spanned<Expr>>),
            Index(Spanned<Expr>),
        }

        let dot_access = just(Token::Dot)
            .ignore_then(ident.clone())
            .map(PostfixOp::Access);

        let bracket_string_access = select! { Token::Str(s) => s }
            .map_with(|s, ex| (s, span_to_range(ex.span())))
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

        let postfix = atom.foldl_with(postfix_op.repeated(), |lhs, op, ex| {
            let span = span_to_range(ex.span());
            let node = match op {
                PostfixOp::Access(name) => Expr::Access(Box::new(lhs), name),
                PostfixOp::Call(args) => Expr::Call(Box::new(lhs), args),
                PostfixOp::Index(idx) => Expr::Index(Box::new(lhs), Box::new(idx)),
            };
            (node, span)
        });

        let unary = choice((
            just(Token::Minus).to(UnaryOp::Neg),
            just(Token::Bang).to(UnaryOp::Not),
        ))
        .repeated()
        .foldr_with(postfix, |op, rhs, ex| {
            (Expr::Unary(op, Box::new(rhs)), span_to_range(ex.span()))
        });

        let mul_op = choice((
            just(Token::Star).to(BinOp::Mul),
            just(Token::Slash).to(BinOp::Div),
            just(Token::Percent).to(BinOp::Mod),
        ));

        let multitive = unary.clone().foldl_with(
            mul_op.then(unary).repeated(),
            |lhs, (op, rhs), ex| {
                (
                    Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span_to_range(ex.span()),
                )
            },
        );

        let add_op = choice((
            just(Token::Plus).to(BinOp::Add),
            just(Token::Minus).to(BinOp::Sub),
        ));

        let additive = multitive.clone().foldl_with(
            add_op.then(multitive).repeated(),
            |lhs, (op, rhs), ex| {
                (
                    Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span_to_range(ex.span()),
                )
            },
        );

        let cmp_op = choice((
            just(Token::NotEq).to(BinOp::Ne),
            just(Token::GtEq).to(BinOp::Ge),
            just(Token::LtEq).to(BinOp::Le),
            just(Token::Eq).to(BinOp::Eq),
            just(Token::Gt).to(BinOp::Gt),
            just(Token::Lt).to(BinOp::Lt),
        ));

        additive.clone().foldl_with(
            cmp_op.then(additive).repeated(),
            |lhs, (op, rhs), ex| {
                (
                    Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span_to_range(ex.span()),
                )
            },
        )
    });

    let bind = ident
        .clone()
        .then_ignore(just(Token::Colon))
        .then(expr.clone());

    #[derive(Clone)]
    enum StmtItem {
        Bind(Bind),
        Body(Spanned<Expr>),
    }

    let stmt_item = choice((
        ident
            .clone()
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
            body.map(|body| Statement { definitions, body })
                .ok_or_else(|| Rich::custom(span, "statement must end with a body expression"))
        })
}
