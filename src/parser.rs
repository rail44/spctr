use crate::token::*;
use nom::{
    branch::alt,
    character::complete::{alpha1, char, digit1, space0},
    combinator::{map, map_res},
    multi::{fold_many0, separated_list},
    sequence::{delimited, pair, separated_pair},
    IResult,
};
use std::str::FromStr;

fn number(input: &str) -> IResult<&str, Primary> {
    let (input, n) = map_res(delimited(space0, digit1, space0), FromStr::from_str)(input)?;
    Ok((input, Primary::Number(n)))
}

fn identifier(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(space0, alpha1, space0)(input)?;
    Ok((input, Primary::Identifier(s.to_string())))
}

fn block(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('{'), statement, char('}'))(input)?;
    Ok((input, Primary::Block(Box::new(s))))
}

fn primary(input: &str) -> IResult<&str, Primary> {
    alt((number, identifier, block))(input)
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

fn bind(input: &str) -> IResult<&str, (String, Additive)> {
    let (input, (label, v)) = separated_pair(alpha1, char(':'), additive)(input)?;
    Ok((input, (label.to_string(), v)))
}

fn definitions(input: &str) -> IResult<&str, Vec<(String, Additive)>> {
    separated_list(char(','), delimited(space0, bind, space0))(input)
}

fn statement(input: &str) -> IResult<&str, Statement> {
    alt((
        map(
            separated_pair(definitions, char(','), additive),
            |(definitions, body)| Statement { definitions, body },
        ),
        map(additive, |body| Statement {
            definitions: Vec::new(),
            body,
        }),
    ))(input)
}

pub fn parse(input: &str) -> IResult<&str, AST> {
    statement(input)
}

#[test]
fn test_definitions() {
    dbg!(definitions("hoge: 1, fuga:2").unwrap());
}
