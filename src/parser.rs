use crate::token::*;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, char, digit1, space0},
    combinator::{all_consuming, map, opt},
    multi::{fold_many0, separated_list},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
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

fn identifier(input: &str) -> IResult<&str, Primary> {
    let (input, s) = alpha1(input)?;
    Ok((input, Primary::Identifier(s.to_string())))
}

fn block(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('{'), statement, char('}'))(input)?;
    Ok((input, Primary::Block(Box::new(s))))
}

fn string(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('"'), take_until("\""), char('"'))(input)?;
    Ok((input, Primary::String(s.to_string())))
}

fn primary(input: &str) -> IResult<&str, Primary> {
    delimited(space0, alt((number, string, identifier, block)), space0)(input)
}

fn multitive(input: &str) -> IResult<&str, Multitive> {
    let (input, left) = primary(input)?;
    let (input, rights) = fold_many0(
        pair(alt((char('*'), char('/'))), primary),
        Vec::new(),
        |mut vec, (op, val)| {
            match op {
                '*' => vec.push(MultitiveRight::Mul(val)),
                '/' => vec.push(MultitiveRight::Div(val)),
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
    let (input, (label, v)) = separated_pair(alpha1, char(':'), expression)(input)?;
    Ok((input, (label.to_string(), v)))
}

fn definitions(input: &str) -> IResult<&str, Vec<(String, Expression)>> {
    separated_list(char(','), delimited(space0, bind, space0))(input)
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
        space0,
        preceded(tag("if"), tuple((expression, expression, expression))),
        space0,
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
