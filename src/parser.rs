use crate::token::*;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, char, digit1, space0},
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

fn identifier(input: &str) -> IResult<&str, Primary> {
    let (input, s) = alpha1(input)?;
    Ok((input, Primary::Identifier(s.to_string())))
}

fn block(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('{'), statement, char('}'))(input)?;
    Ok((input, Primary::Block(Box::new(s))))
}

fn arrow(input: &str) -> IResult<&str, &str> {
    delimited(space0, tag("=>"), space0)(input)
}

fn call(input: &str) -> IResult<&str, OperationRight> {
    map(
        delimited(char('('), separated_list(char(','), expression), char(')')),
        |args| OperationRight::Call(args),
    )(input)
}

fn args(input: &str) -> IResult<&str, Vec<String>> {
    map(
        delimited(
            char('('),
            separated_list(char(','), delimited(space0, alpha1, space0)),
            char(')'),
        ),
        |args: Vec<&str>| args.into_iter().map(|s| s.to_string()).collect(),
    )(input)
}

fn function(input: &str) -> IResult<&str, Primary> {
    let (input, s) = pair(args, preceded(arrow, expression))(input)?;
    Ok((input, Primary::Function(s.0, Box::new(s.1))))
}

fn string(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('"'), take_until("\""), char('"'))(input)?;
    Ok((input, Primary::String(s.to_string())))
}

fn struct_(input: &str) -> IResult<&str, Primary> {
    let (input, s) = delimited(char('{'), definitions, char('}'))(input)?;
    Ok((input, Primary::Struct(s)))
}

fn array(input: &str) -> IResult<&str, Primary> {
    map(
        delimited(char('['), separated_list(char(','), expression), char(']')),
        |items| Primary::Array(items),
    )(input)
}

fn primary(input: &str) -> IResult<&str, Primary> {
    alt((number, string, identifier, block, array, function, struct_))(input)
}

fn access(input: &str) -> IResult<&str, OperationRight> {
    map(preceded(char('.'), alpha1), |prop: &str| {
        OperationRight::Access(prop.to_string())
    })(input)
}

fn operation(input: &str) -> IResult<&str, Operation> {
    let (input, left) = preceded(space0, primary)(input)?;
    let (input, rights) = terminated(many0(alt((access, call))), space0)(input)?;
    Ok((input, Operation { left, rights }))
}

fn multitive(input: &str) -> IResult<&str, Multitive> {
    let (input, left) = operation(input)?;
    let (input, rights) = fold_many0(
        pair(alt((char('*'), char('/'))), operation),
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
