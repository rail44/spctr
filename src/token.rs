use failure::err_msg;
use pest::iterators::{Pair, Pairs};
use pest::Parser as PestParser;
use pest_derive::Parser as PestParser;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;

#[derive(PestParser)]
#[grammar = "grammar.pest"]
struct Parser;

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Comparison(Comparison),
    Function(Vec<String>, Box<Expression>),
}

impl TryInto<String> for Expression {
    type Error = failure::Error;
    fn try_into(self) -> Result<String, Self::Error> {
        if let Expression::Comparison(e) = self {
            return e.try_into();
        }
        Err(err_msg(format!("{:?}", self)))
    }
}

impl TryFrom<Pair<'_, Rule>> for Expression {
    type Error = failure::Error;
    fn try_from(pair: Pair<Rule>) -> Result<Self, Self::Error> {
        use Expression::*;
        match pair.as_rule() {
            Rule::comparison => Ok(Comparison(pair.into_inner().try_into()?)),
            Rule::function => {
                let mut v: Vec<Pair<Rule>> = pair.into_inner().collect();
                let expression = v.pop().unwrap().into_inner().next().unwrap().try_into()?;
                let arg_names = v.into_iter().map(|p| p.as_str().to_string()).collect();
                Ok(Function(arg_names, Box::new(expression)))
            }
            _ => Err(err_msg(format!("{:?}", pair))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub binds: HashMap<String, Expression>,
    pub expressions: Vec<Expression>,
}

impl FromStr for Source {
    type Err = failure::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Source::try_from(Parser::parse(Rule::source, s)?)?)
    }
}

impl TryFrom<Pairs<'_, Rule>> for Source {
    type Error = failure::Error;
    fn try_from(pairs: Pairs<Rule>) -> Result<Self, Self::Error> {
        let mut binds = HashMap::new();
        let mut expressions = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::bind => {
                    let mut inner = pair.into_inner();
                    let name = inner.next().unwrap().as_str();
                    let expression = inner
                        .next()
                        .unwrap()
                        .into_inner()
                        .next()
                        .unwrap()
                        .try_into()?;
                    binds.insert(name.to_string(), expression);
                }
                Rule::expression => {
                    expressions.push(Expression::try_from(pair.into_inner().next().unwrap())?)
                }
                _ => return Err(err_msg(format!("{:?}", pair))),
            }
        }
        Ok(Source { binds, expressions })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub left: Additive,
    pub rights: Vec<ComparisonRight>,
}

impl TryFrom<Pairs<'_, Rule>> for Comparison {
    type Error = failure::Error;

    fn try_from(mut pairs: Pairs<Rule>) -> Result<Self, Self::Error> {
        let left = Additive::try_from(pairs.next().unwrap().into_inner())?;
        let mut rights = vec![];

        for pair in pairs {
            rights.push(ComparisonRight::try_from(pair)?);
        }

        Ok(Self { left, rights })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonKind {
    Equal,
    NotEqual,
}

impl TryFrom<&Pair<'_, Rule>> for ComparisonKind {
    type Error = failure::Error;

    fn try_from(pair: &Pair<Rule>) -> Result<Self, Self::Error> {
        use ComparisonKind::*;
        match pair.as_rule() {
            Rule::equal => Ok(Equal),
            Rule::not_equal => Ok(NotEqual),
            _ => Err(err_msg(format!("{:?}", pair))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRight {
    pub kind: ComparisonKind,
    pub value: Additive,
}

impl TryFrom<Pair<'_, Rule>> for ComparisonRight {
    type Error = failure::Error;

    fn try_from(pair: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        let kind = ComparisonKind::try_from(&pair)?;
        let value = Additive::try_from(pair.into_inner().next().unwrap().into_inner())?;

        Ok(Self { kind, value })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Additive {
    pub left: Multitive,
    pub rights: Vec<AdditiveRight>,
}

impl TryInto<String> for Comparison {
    type Error = failure::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        if let Primary::Evaluation(e) = self.left.left.left {
            return Ok(e.left);
        }
        Err(err_msg(format!("{:?}", self)))
    }
}

impl TryFrom<Pairs<'_, Rule>> for Additive {
    type Error = failure::Error;

    fn try_from(mut pairs: Pairs<Rule>) -> Result<Self, Self::Error> {
        let left = Multitive::try_from(pairs.next().unwrap().into_inner())?;
        let mut rights = vec![];

        for pair in pairs {
            rights.push(AdditiveRight::try_from(pair)?);
        }

        Ok(Self { left, rights })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdditiveKind {
    Add,
    Sub,
}

impl TryFrom<&Pair<'_, Rule>> for AdditiveKind {
    type Error = failure::Error;

    fn try_from(pair: &Pair<Rule>) -> Result<Self, Self::Error> {
        use AdditiveKind::*;
        match pair.as_rule() {
            Rule::add => Ok(Add),
            Rule::sub => Ok(Sub),
            _ => Err(err_msg(format!("{:?}", pair))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdditiveRight {
    pub kind: AdditiveKind,
    pub value: Multitive,
}

impl TryFrom<Pair<'_, Rule>> for AdditiveRight {
    type Error = failure::Error;

    fn try_from(pair: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        let kind = AdditiveKind::try_from(&pair)?;
        let value = Multitive::try_from(pair.into_inner().next().unwrap().into_inner())?;

        Ok(Self { kind, value })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Multitive {
    pub left: Primary,
    pub rights: Vec<MultitiveRight>,
}

impl TryFrom<Pairs<'_, Rule>> for Multitive {
    type Error = failure::Error;

    fn try_from(mut pairs: Pairs<Rule>) -> Result<Self, Self::Error> {
        let left = Primary::try_from(pairs.next().unwrap())?;
        let mut rights = vec![];

        for pair in pairs {
            rights.push(MultitiveRight::try_from(pair)?);
        }

        Ok(Self { left, rights })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MultitiveKind {
    Mul,
    Div,
    Surplus,
}

impl TryFrom<&Pair<'_, Rule>> for MultitiveKind {
    type Error = failure::Error;

    fn try_from(pair: &Pair<Rule>) -> Result<Self, Self::Error> {
        use MultitiveKind::*;
        match pair.as_rule() {
            Rule::mul => Ok(Mul),
            Rule::div => Ok(Div),
            Rule::surplus => Ok(Surplus),
            _ => Err(err_msg(format!("{:?}", pair))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultitiveRight {
    pub kind: MultitiveKind,
    pub value: Primary,
}

impl TryFrom<Pair<'_, Rule>> for MultitiveRight {
    type Error = failure::Error;

    fn try_from(pair: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        let kind = MultitiveKind::try_from(&pair)?;
        let value = Primary::try_from(pair.into_inner().next().unwrap())?;

        Ok(Self { kind, value })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primary {
    Number(f64),
    String(String),
    Parenthesis(Box<Expression>),
    Block(Box<Source>),
    List(Vec<Expression>),
    Evaluation(Evaluation),
    If(Box<Comparison>, Box<Expression>, Box<Expression>),
}

impl TryFrom<Pair<'_, Rule>> for Primary {
    type Error = failure::Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self, Self::Error> {
        match pair.as_rule() {
            Rule::parenthesis => Ok(Primary::Parenthesis(Box::new(
                pair.into_inner()
                    .next()
                    .unwrap()
                    .into_inner()
                    .next()
                    .unwrap()
                    .try_into()?,
            ))),
            Rule::number => Ok(Primary::Number(pair.as_str().parse().unwrap())),
            Rule::string_literal => Ok(Primary::String(pair.into_inner().next().unwrap().as_str().replace("\\\"", "\"").to_string())),
            Rule::list => {
                let mut expressions = vec![];
                for member in pair.into_inner() {
                    expressions.push(member.into_inner().next().unwrap().try_into()?)
                }
                Ok(Primary::List(expressions))
            }
            Rule::block => Ok(Primary::Block(Box::new(Source::try_from(
                pair.into_inner(),
            )?))),
            Rule::evaluation => Ok(Primary::Evaluation(Evaluation::try_from(
                pair.into_inner(),
            )?)),
            Rule::_if => {
                let mut inner = pair.into_inner();
                Ok(Primary::If(
                    Box::new(
                        inner
                            .next()
                            .unwrap()
                            .into_inner()
                            .next()
                            .unwrap()
                            .into_inner()
                            .try_into()?,
                    ),
                    Box::new(
                        inner
                            .next()
                            .unwrap()
                            .into_inner()
                            .next()
                            .unwrap()
                            .try_into()?,
                    ),
                    Box::new(
                        inner
                            .next()
                            .unwrap()
                            .into_inner()
                            .next()
                            .unwrap()
                            .try_into()?,
                    ),
                ))
            }
            _ => Err(err_msg(format!("{:?}", pair))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub left: String,
    pub rights: Vec<EvaluationRight>,
}

impl TryFrom<Pairs<'_, Rule>> for Evaluation {
    type Error = failure::Error;

    fn try_from(mut pairs: Pairs<Rule>) -> Result<Self, Self::Error> {
        let left = pairs.next().unwrap().as_str().to_string();
        let mut rights = vec![];
        for pair in pairs {
            rights.push(EvaluationRight::try_from(pair)?);
        }
        Ok(Evaluation { left, rights })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationRight {
    Call(Expression),
    Access(String),
}

impl TryFrom<Pair<'_, Rule>> for EvaluationRight {
    type Error = failure::Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self, Self::Error> {
        use EvaluationRight::*;
        match pair.as_rule() {
            Rule::calling => Ok(Call(
                pair.into_inner()
                    .next()
                    .unwrap()
                    .into_inner()
                    .next()
                    .unwrap()
                    .try_into()?,
            )),
            Rule::identify => Ok(Access(pair.as_str().to_string())),
            _ => Err(err_msg(format!("{:?}", pair))),
        }
    }
}

#[test]
fn test_parsing_identify() {
    Parser::parse(Rule::identify, "hoge").unwrap();
}

#[test]
fn test_parsing_comparison() {
    Parser::parse(Rule::comparison, "1 = 0").unwrap();
}

#[test]
fn test_parsing_additive() {
    Parser::parse(Rule::additive, "2").unwrap();
    Parser::parse(Rule::additive, "(2 + i) / j * 3 - k").unwrap();
}

#[test]
fn test_parsing_bind() {
    Parser::parse(Rule::bind, "hoge: 2").unwrap();
    Parser::parse(Rule::bind, "hoge: 2 / 1").unwrap();
}

#[test]
fn test_parsing_evaluation() {
    Parser::parse(Rule::evaluation, "hoge(1 * 2 + 3)").unwrap();
}

#[test]
fn test_parsing_string() {
    Parser::parse(Rule::string, "\"hoge fuga\"").unwrap();
    Parser::parse(Rule::string, "\"\"").unwrap();
}

#[test]
fn test_parsing_source_1() {
    let ast = "i";
    Parser::parse(Rule::source, ast).unwrap();
    Source::try_from(Parser::parse(Rule::source, ast).unwrap()).unwrap();
}

#[test]
fn test_parsing_source_2() {
    let ast = "1 + 2";
    Source::try_from(Parser::parse(Rule::source, ast).unwrap()).unwrap();
}

#[test]
fn test_parsing_source_3() {
    let ast = "i: j";
    Source::try_from(Parser::parse(Rule::source, ast).unwrap()).unwrap();
}

#[test]
fn test_parsing_source_4() {
    let ast = "i: j / 2,
j: 5,
k: k + 1,
i * (j + 3) + (j / i)";
    Source::try_from(Parser::parse(Rule::source, ast).unwrap()).unwrap();
}

#[test]
fn test_parsing_source_5() {
    let ast = "fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz \"fizz\" \"\",
  buzz: if is_buzz \"buzz\" \"\",

  fizz.concat(buzz)
},
Array.range({start: 1, end: 100}).map(fizzbuzz)";
    Source::try_from(Parser::parse(Rule::source, ast).unwrap()).unwrap();
}

#[test]
fn test_parsing_source_6() {
    let ast = "i(1)";
    Source::try_from(Parser::parse(Rule::source, ast).unwrap()).unwrap();
}
