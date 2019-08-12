use pest::Parser as PestParser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser as PestParser;
use std::str::FromStr;
use std::collections::HashMap;

#[derive(PestParser)]
#[grammar = "grammar.pest"]
struct Parser;

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Comparison(Comparison),
    Function(Vec<String>, Box<Expression>),
}

impl Into<String> for Expression {
    fn into(self) -> String {
        if let Expression::Comparison(e) = self {
            return e.into();
        }
        panic!("{:?}", self);
    }
}

impl From<Pair<'_, Rule>> for Expression {
    fn from(pair: Pair<Rule>) -> Self {
        use Expression::*;
        match pair.as_rule() {
            Rule::comparison => Comparison(pair.into_inner().into()),
            Rule::function => {
                let mut v: Vec<Pair<Rule>> = pair.into_inner().collect();
                let expression = v.pop().unwrap().into_inner().next().unwrap().into();
                let mut arg_names = vec![];
                for pair in v {
                    arg_names.push(pair.as_str().to_string());
                }
                Function(arg_names, Box::new(expression))
            }
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub binds: HashMap<String, Expression>,
    pub expressions: Vec<Expression>
}

impl FromStr for Source {
    type Err = pest::error::Error<Rule>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Source::from(Parser::parse(Rule::source, s)?))
    }
}

impl From<Pairs<'_, Rule>> for Source {
    fn from(pairs: Pairs<Rule>) -> Self {
        let mut binds = HashMap::new();
        let mut expressions = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::bind => {
                    let mut inner = pair.into_inner();
                    let name = inner.next().unwrap().as_str();
                    let expression = inner.next().unwrap().into_inner().next().unwrap().into();
                    binds.insert(name.to_string(), expression);
                }
                Rule::expression => expressions.push(Expression::from(pair.into_inner().next().unwrap())),
                _ => unreachable!("{:?}", pair)
            }
        }
        Source {
            binds,
            expressions
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub left: Additive,
    pub rights: Vec<ComparisonRight>
}

impl From<Pairs<'_, Rule>> for Comparison {
    fn from(mut pairs: Pairs<Rule>) -> Self {
        let left = Additive::from(pairs.next().unwrap().into_inner());
        let mut rights = vec![];

        for pair in pairs {
            rights.push(ComparisonRight::from(pair));
        }

        Self {
            left,
            rights
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonKind {
    Equal,
    NotEqual
}

impl From<&Pair<'_, Rule>> for ComparisonKind {
    fn from(pair: &Pair<Rule>) -> Self {
        use ComparisonKind::*;
        match pair.as_rule() {
            Rule::equal => Equal,
            Rule::not_equal => NotEqual,
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRight {
    pub kind: ComparisonKind,
    pub value: Additive
}

impl From<Pair<'_, Rule>> for ComparisonRight {
    fn from(pair: Pair<'_, Rule>) -> Self {
        let kind = ComparisonKind::from(&pair);
        let value = Additive::from(pair.into_inner().next().unwrap().into_inner());

        Self {
            kind,
            value
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Additive {
    pub left: Multitive,
    pub rights: Vec<AdditiveRight>
}

impl Into<String> for Comparison {
    fn into(self) -> String {
        if let Primary::Evaluation(e) = self.left.left.left {
            return e.left
        }
        panic!("{:?}", self);
    }
}

impl From<Pairs<'_, Rule>> for Additive {
    fn from(mut pairs: Pairs<Rule>) -> Self {
        let left = Multitive::from(pairs.next().unwrap().into_inner());
        let mut rights = vec![];

        for pair in pairs {
            rights.push(AdditiveRight::from(pair));
        }

        Self {
            left,
            rights
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdditiveKind {
    Add,
    Sub
}

impl From<&Pair<'_, Rule>> for AdditiveKind {
    fn from(pair: &Pair<Rule>) -> Self {
        use AdditiveKind::*;
        match pair.as_rule() {
            Rule::add => Add,
            Rule::sub => Sub,
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdditiveRight {
    pub kind: AdditiveKind,
    pub value: Multitive
}

impl From<Pair<'_, Rule>> for AdditiveRight {
    fn from(pair: Pair<'_, Rule>) -> Self {
        let kind = AdditiveKind::from(&pair);
        let value = Multitive::from(pair.into_inner().next().unwrap().into_inner());

        Self {
            kind,
            value
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Multitive {
    pub left: Primary,
    pub rights: Vec<MultitiveRight>
}

impl From<Pairs<'_, Rule>> for Multitive {
    fn from(mut pairs: Pairs<Rule>) -> Self {
        let left = Primary::from(pairs.next().unwrap());
        let mut rights = vec![];

        for pair in pairs {
            rights.push(MultitiveRight::from(pair));
        }

        Self {
            left,
            rights
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MultitiveKind {
    Mul,
    Div,
    Surplus
}

impl From<&Pair<'_, Rule>> for MultitiveKind {
    fn from(pair: &Pair<Rule>) -> Self {
        use MultitiveKind::*;
        match pair.as_rule() {
            Rule::mul => Mul,
            Rule::div => Div,
            Rule::surplus => Surplus,
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultitiveRight {
    pub kind: MultitiveKind,
    pub value: Primary
}

impl From<Pair<'_, Rule>> for MultitiveRight {
    fn from(pair: Pair<'_, Rule>) -> Self {
        let kind = MultitiveKind::from(&pair);
        let value = Primary::from(pair.into_inner().next().unwrap());

        Self {
            kind,
            value
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primary {
    Number(f64),
    String(String),
    Parenthesis(Box<Expression>),
    Block(Box<Source>),
    Evaluation(Evaluation),
    If(Box<Comparison>, Box<Expression>, Box<Expression>)
}

impl From<Pair<'_, Rule>> for Primary {
    fn from(pair: Pair<Rule>) -> Self {
        match pair.as_rule() {
            Rule::parenthesis => Primary::Parenthesis(Box::new(pair.into_inner().next().unwrap().into_inner().next().unwrap().into())),
            Rule::number => Primary::Number(pair.as_str().parse().unwrap()),
            Rule::string => Primary::String(pair.as_str().to_string()),
            Rule::block => Primary::Block(Box::new(Source::from(pair.into_inner()))),
            Rule::evaluation => Primary::Evaluation(Evaluation::from(pair.into_inner())),
            Rule::_if => {
                let mut inner = pair.into_inner();
                Primary::If(
                    Box::new(inner.next().unwrap().into_inner().next().unwrap().into_inner().into()),
                    Box::new(inner.next().unwrap().into_inner().next().unwrap().into()),
                    Box::new(inner.next().unwrap().into_inner().next().unwrap().into())
                )
            }
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub left: String,
    pub rights: Vec<EvaluationRight>,
}

impl From<Pairs<'_, Rule>> for Evaluation {
    fn from(mut pairs: Pairs<Rule>) -> Self {
        let left = pairs.next().unwrap().as_str().to_string();
        let mut rights = vec![];
        for pair in pairs {
            rights.push(EvaluationRight::from(pair));
        }
        Evaluation {
            left,
            rights
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationRight {
    Call(Expression),
    Access(String),
}

impl From<Pair<'_, Rule>> for EvaluationRight {
    fn from(pair: Pair<Rule>) -> Self {
        use EvaluationRight::*;
        match pair.as_rule() {
            Rule::calling => Call(pair.into_inner().next().unwrap().into_inner().next().unwrap().into()),
            Rule::identify => Access(pair.as_str().to_string()),
            _ => unreachable!("{:?}", pair)
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
    let source = Source::from_str(ast).unwrap();
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}

#[test]
fn test_parsing_source_2() {
    let ast = "1 + 2";
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}

#[test]
fn test_parsing_source_3() {
    let ast = "i: j";
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}

#[test]
fn test_parsing_source_4() {
    let ast = "i: j / 2,
j: 5,
k: k + 1,
i * (j + 3) + (j / i)";
    Source::from(Parser::parse(Rule::source, ast).unwrap());
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
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}

#[test]
fn test_parsing_source_6() {
    let ast = "i(1)";
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}
