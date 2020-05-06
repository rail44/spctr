use nom::{
  IResult,
  branch::alt,
  combinator::{map, map_res, iterator},
  character::complete::{char, space0, digit1, alpha1},
  sequence::{pair, delimited, separated_pair, preceded},
  multi::separated_list
};
use crate::token::*;
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
    let mut iter = iterator(
        input,
        pair(alt((char('*'), char('/'))), primary)
    );
    let rights = iter.map(|(op, val)| match op {
            '*' => MultitiveRight::Mul(val),
            '/' => MultitiveRight::Div(val),
            _ => unreachable!()
        }).collect();
    iter.finish()?;
    Ok((input, Multitive { left, rights }))
}

fn additive(input: &str) -> IResult<&str, Additive> {
    let (input, left) = multitive(input)?;
    let mut iter = iterator(
        input,
        pair(alt((char('+'), char('-'))), multitive)
    );
    let rights = iter.map(|(op, val)| match op {
            '+' => AdditiveRight::Add(val),
            '-' => AdditiveRight::Sub(val),
            _ => unreachable!()
        }).collect();
    iter.finish()?;
    Ok((input, Additive { left, rights }))
}

fn bind(input: &str) -> IResult<&str, (String, Additive)> {
    let (input, (label, v)) = separated_pair(alpha1, char(':'), additive)(input)?;
    Ok((input, (label.to_string(), v)))
}

fn definitions(input: &str) -> IResult<&str, Vec<(String, Additive)>> {
    separated_list(char(','), bind)(input)
}

fn statement(input: &str) -> IResult<&str, Statement> {
    alt((
        map(
            pair(
                definitions,
                preceded(char(','), additive)
            ),
            |(definitions, body)| Statement { definitions, body }
        ),
        map(additive, |body| Statement { definitions: Vec::new(), body })
    ))(input)
}

pub fn parse(input: &str) -> IResult<&str, AST> {
    statement(input)
}
