use pest::Parser as PestParser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser as PestParser;
use std::collections::HashMap;

#[derive(Debug)]
enum Type {
    Number(f64),
    String(String)
}

#[derive(PestParser)]
#[grammar = "grammar.pest"]
struct Parser;

#[derive(Debug)]
struct Additive {
    left: Multitive,
    rights: Vec<AdditiveRight>
}

impl Additive {
    fn eval(self) -> Type {
        if let Type::Number(mut base) = self.left.eval() {
            for right in self.rights {
                use AdditiveKind::*;
                if let Type::Number(value) = right.value.eval() {
                    match right.kind {
                        Add => base += value,
                        Sub => base -= value,
                    }
                    continue;
                }
                panic!("not a number");
            }
            return Type::Number(base);
        }
        panic!("not a number");
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

#[derive(Debug)]
enum AdditiveKind {
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

#[derive(Debug)]
struct AdditiveRight {
    kind: AdditiveKind,
    value: Multitive
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

#[derive(Debug)]
struct Multitive {
    left: Primary,
    rights: Vec<MultitiveRight>
}

impl Multitive {
    fn eval(self) -> Type {
        if let Type::Number(mut base) = self.left.eval() {
            for right in self.rights {
                if let Type::Number(value) = right.value.eval() {
                    use MultitiveKind::*;
                    match right.kind {
                        Mul => base *= value,
                        Div => base /= value,
                        Surplus => base = base % value,
                    }
                    continue;
                }
                panic!("not a number");
            }
            return Type::Number(base);
        }
        panic!("not a number");
    }
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

#[derive(Debug)]
enum MultitiveKind {
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

#[derive(Debug)]
struct MultitiveRight {
    kind: MultitiveKind,
    value: Primary
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

#[derive(Debug)]
enum Primary {
    Number(f64),
    Parenthesis(Box<Additive>)
}

impl Primary {
    fn eval(self) -> Type {
        use Primary::*;
        match self {
            Number(f) => Type::Number(f),
            Parenthesis(a) => a.eval()
        }
    }
}

impl From<Pair<'_, Rule>> for Primary {
    fn from(pair: Pair<Rule>) -> Self {
        use Primary::*;
        match pair.as_rule() {
            Rule::parenthesis => Parenthesis(Box::new(Additive::from(pair.into_inner().next().unwrap().into_inner()))),
            Rule::number => Number(pair.as_str().parse().unwrap()),
            _ => unreachable!("{:?}", pair)
        }
    }
}

fn main() {
    let additive = Additive::from(Parser::parse(Rule::source, "2 * (5 % 2) + 4 / 2").unwrap().next().unwrap().into_inner());
    println!("{:?}", additive);
    println!("{:?}", additive.eval());

    let ast = "hoge: {
  foo: 12
  bar: 23
  baz: foo + bar
}

  hoge.baz
";
    let mut source = Parser::parse(Rule::source, ast).unwrap();
    println!("{:?}", source);
    let additive = Additive::from(source.next().unwrap().into_inner());
    println!("{:?}", additive);
    println!("{:?}", additive.eval());
}

#[test]
fn test_parsing() {
    Parser::parse(Rule::identify, "hoge").unwrap();

    Parser::parse(Rule::comparison, "1 = 0").unwrap();

    Parser::parse(Rule::additive, "2").unwrap();
    Parser::parse(Rule::additive, "(2 + i) / j * 3 - k").unwrap();

    Parser::parse(Rule::bind, "hoge: 2").unwrap();
    Parser::parse(Rule::bind, "hoge: 2 / 1").unwrap();

    Parser::parse(Rule::evaluation, "hoge(1 * 2 + 3)").unwrap();

    Parser::parse(Rule::string, "\"hoge fuga\"").unwrap();
    Parser::parse(Rule::string, "\"\"").unwrap();

    let ast = "i: j";
    Parser::parse(Rule::source, ast).unwrap();

    let ast = "i: j / 2";
    Parser::parse(Rule::source, ast).unwrap();

    let ast = "i: j / 2
j: 5
k: k + 1

i * (j + 3) + (j / i)";
    Parser::parse(Rule::source, ast).unwrap();

    let ast = "fizzbuzz: {
  is_fizz: i % 3 = 0
  is_buzz: i % 5 = 0
  fizz: if is_fizz \"fizz\" \"\"
  buzz: if is_buzz \"buzz\" \"\"

  fizz.concat(buzz)
}

range({min: 1 max: 100}).map({i: fizzbuzz})";
    Parser::parse(Rule::source, ast).unwrap();
}
