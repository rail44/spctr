use nom::{
  IResult,
  branch::alt,
  combinator::{map_res, iterator},
  character::complete::{char, space0, digit1},
  sequence::{pair, delimited},
};
use std::str::FromStr;

pub type AST = Additive;

#[derive(Clone, Debug)]
pub struct Additive {
    pub left: Multitive,
    pub rights: Vec<AdditiveRight>
}

#[derive(Clone, Debug)]
pub enum AdditiveRight {
    Add(Multitive),
    Sub(Multitive)
}

#[derive(Clone, Debug)]
pub struct Multitive {
    pub left: f64,
    pub rights: Vec<MultitiveRight>
}

#[derive(Clone, Debug)]
pub enum MultitiveRight {
    Mul(f64),
    Div(f64),
}

pub fn number(input: &str) -> IResult<&str, f64> {
    map_res(delimited(space0, digit1, space0), FromStr::from_str)(input)
}

pub fn multitive(input: &str) -> IResult<&str, Multitive> {
    let (input, left) = number(input)?;
    let mut iter = iterator(
        input,
        pair(alt((char('*'), char('/'))), number)
    );
    let rights = iter.map(|(op, val)| match op {
            '*' => MultitiveRight::Mul(val),
            '/' => MultitiveRight::Div(val),
            _ => unreachable!()
        }).collect();
    iter.finish()?;
    Ok((input, Multitive { left, rights }))
}

pub fn additive(input: &str) -> IResult<&str, Additive> {
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

pub fn parse(input: &str) -> IResult<&str, Additive> {
    additive(input)
}
