use crate::token::*;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{char, digit1, multispace0},
    combinator::{all_consuming, map, opt},
    multi::{fold_many0, many0, separated_list},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};
use std::str::FromStr;

fn number(input: &str) -> IResult<&str, Primary> {
    let (input, n) = map(pair(opt(char('-')), digit1), |(neg, v)| {
        let n: f64 = FromStr::from_str(v).unwrap();
        if neg.is_some() {
            return -n;
        }
        n
    })(input)?;
    Ok((input, Primary::Number(n)))
}

fn identifier(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|chr: char| chr.is_alphabetic() || chr == '_'),
        |s: &str| s.to_string(),
    )(input)
}

fn variable(input: &str) -> IResult<&str, Primary> {
    map(identifier, Primary::Variable)(input)
}

fn block(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('{'), statement, char('}'))(input)?;
    Ok((input, Primary::Block(Box::new(s))))
}

fn arrow(input: &str) -> IResult<&str, &str> {
    delimited(multispace0, tag("=>"), multispace0)(input)
}

fn call(input: &str) -> IResult<&str, OperationRight> {
    map(
        delimited(char('('), separated_list(char(','), expression), char(')')),
        OperationRight::Call,
    )(input)
}

fn index(input: &str) -> IResult<&str, OperationRight> {
    map(
        delimited(char('['), expression, char(']')),
        OperationRight::Index,
    )(input)
}

fn args(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        char('('),
        separated_list(char(','), delimited(multispace0, identifier, multispace0)),
        char(')'),
    )(input)
}

fn function(input: &str) -> IResult<&str, Primary> {
    let (input, s) = pair(args, preceded(arrow, expression))(input)?;
    Ok((input, Primary::Function(s.0, Box::new(s.1))))
}

fn string(input: &str) -> IResult<&str, String> {
    map(
        delimited(char('"'), take_until("\""), char('"')),
        String::from
    )(input)
}

fn string_literal(input: &str) -> IResult<&str, Primary> {
    map(
        string,
        Primary::String
    )(input)
}

fn struct_(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('{'), definitions, char('}'))(input)?;
    Ok((input, Primary::Struct(s)))
}

fn list(input: &str) -> IResult<&str, Primary> {
    map(
        delimited(char('['), separated_list(char(','), expression), char(']')),
        Primary::List,
    )(input)
}

fn null(input: &str) -> IResult<&str, Primary> {
    map(tag("null"), |_| Primary::Null)(input)
}

fn primary(input: &str) -> IResult<&str, Primary> {
    alt((
        number, string_literal, block, list, function, struct_, null, variable,
    ))(input)
}

fn access(input: &str) -> IResult<&str, OperationRight> {
    map(
        alt((
            preceded(char('.'), identifier),
            delimited(char('['), string, char(']')),
        )),
        OperationRight::Access
    )(input)
}

fn operation(input: &str) -> IResult<&str, Operation> {
    let (input, left) = preceded(multispace0, primary)(input)?;
    let (input, rights) = terminated(many0(alt((access, call, index))), multispace0)(input)?;
    Ok((input, Operation { left, rights }))
}

fn multitive(input: &str) -> IResult<&str, Multitive> {
    let (input, left) = operation(input)?;
    let (input, rights) = fold_many0(
        pair(alt((char('*'), char('/'), char('%'))), operation),
        Vec::new(),
        |mut vec, (op, val)| {
            match op {
                '*' => vec.push(MultitiveRight::Mul(val)),
                '/' => vec.push(MultitiveRight::Div(val)),
                '%' => vec.push(MultitiveRight::Surplus(val)),
                _ => unreachable!(),
            };
            vec
        },
    )(input)?;
    Ok((input, Multitive { left, rights }))
}

fn additive(input: &str) -> IResult<&str, Additive> {
    let (input, left) = multitive(input)?;
    let (input, rights) = fold_many0(
        pair(alt((char('+'), char('-'))), multitive),
        Vec::new(),
        |mut vec, (op, val)| {
            match op {
                '+' => vec.push(AdditiveRight::Add(val)),
                '-' => vec.push(AdditiveRight::Sub(val)),
                _ => unreachable!(),
            };
            vec
        },
    )(input)?;
    Ok((input, Additive { left, rights }))
}

fn comparison(input: &str) -> IResult<&str, Expression> {
    let (input, left) = additive(input)?;
    let (input, rights) = fold_many0(
        pair(alt((tag("="), tag("!="))), additive),
        Vec::new(),
        |mut vec, (op, val)| {
            match op {
                "=" => vec.push(ComparisonRight::Equal(val)),
                "!=" => vec.push(ComparisonRight::NotEqual(val)),
                _ => unreachable!(),
            };
            vec
        },
    )(input)?;
    Ok((input, Expression::Comparison(Comparison { left, rights })))
}

fn bind(input: &str) -> IResult<&str, (String, Expression)> {
    let (input, (label, v)) = separated_pair(identifier, char(':'), expression)(input)?;
    Ok((input, (label, v)))
}

fn definitions(input: &str) -> IResult<&str, Vec<(String, Expression)>> {
    separated_list(char(','), delimited(multispace0, bind, multispace0))(input)
}

fn statement(input: &str) -> IResult<&str, Statement> {
    alt((
        map(
            separated_pair(definitions, char(','), expression),
            |(definitions, body)| Statement { definitions, body },
        ),
        map(expression, |body| Statement {
            definitions: Vec::new(),
            body,
        }),
    ))(input)
}

fn if_(input: &str) -> IResult<&str, Expression> {
    let (input, (cond, cons, alt)) = delimited(
        multispace0,
        preceded(tag("if"), tuple((expression, expression, expression))),
        multispace0,
    )(input)?;
    Ok((
        input,
        Expression::If {
            cond: Box::new(cond),
            cons: Box::new(cons),
            alt: Box::new(alt),
        },
    ))
}

fn expression(input: &str) -> IResult<&str, Expression> {
    alt((if_, comparison))(input)
}

pub fn parse(input: &str) -> IResult<&str, AST> {
    all_consuming(statement)(input)
}

#[test]
fn test_definitions() {
    dbg!(definitions("hoge: 1, fuga:2").unwrap());
}
