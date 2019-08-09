use pest::Parser as PestParser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser as PestParser;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
enum Type {
    Number(f64),
    String(String),
    Block(Source)
}

#[derive(PestParser)]
#[grammar = "grammar.pest"]
struct Parser;

#[derive(Debug, Clone)]
struct Env {
    binds: HashMap<String, Additive>,
    evaluated: HashMap<String, Type>,
    parent: Option<Rc<RefCell<Env>>>
}

impl Env {
    fn get_value(&mut self, name: &str) -> Type {
        self.binds.remove(name).unwrap().eval(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Source {
    binds: HashMap<String, Additive>,
    expressions: Vec<Additive>
}

impl Source {
    fn eval(mut self) -> Type {
        let mut env = Env {
            binds: self.binds,
            evaluated: HashMap::new(),
            parent: None,
        };
        self.expressions.pop().unwrap().eval(&mut env)
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
                    let expression = Additive::from(inner.next().unwrap().into_inner());
                    binds.insert(name.to_string(), expression);
                }
                Rule::additive => expressions.push(Additive::from(pair.into_inner())),
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
struct Additive {
    left: Multitive,
    rights: Vec<AdditiveRight>
}

impl Additive {
    fn eval(self, env: &mut Env) -> Type {
        let left = self.left.eval(env);

        if self.rights.len() == 0 {
            return  left;
        }

        if let Type::Number(mut base) = left {
            for right in self.rights {
                use AdditiveKind::*;
                if let Type::Number(value) = right.value.eval(env) {
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
struct Multitive {
    left: Primary,
    rights: Vec<MultitiveRight>
}

impl Multitive {
    fn eval(self, env: &mut Env) -> Type {
        let left = self.left.clone().eval(env);

        if self.rights.len() == 0 {
            return  left;
        }

        if let Type::Number(mut base) = left {
            for right in self.rights {
                if let Type::Number(value) = right.value.clone().eval(env) {
                    use MultitiveKind::*;
                    match right.kind {
                        Mul => base *= value,
                        Div => base /= value,
                        Surplus => base = base % value,
                    }
                    continue;
                }
                panic!("not a number: {:?}", right);
            }
            return Type::Number(base);
        }
        panic!("not a number: {:?}", self.left.clone());
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
enum Primary {
    Number(f64),
    Parenthesis(Box<Additive>),
    Block(Box<Source>),
    Evaluation(Evaluation)
}

impl Primary {
    fn eval(self, env: &mut Env) -> Type {
        use Primary::*;
        match self {
            Number(f) => Type::Number(f),
            Parenthesis(a) => a.eval(env),
            Block(s) => Type::Block(*s),
            Evaluation(e) => e.eval(env)
        }
    }
}

impl From<Pair<'_, Rule>> for Primary {
    fn from(pair: Pair<Rule>) -> Self {
        match pair.as_rule() {
            Rule::parenthesis => Primary::Parenthesis(Box::new(Additive::from(pair.into_inner().next().unwrap().into_inner()))),
            Rule::number => Primary::Number(pair.as_str().parse().unwrap()),
            Rule::block => Primary::Block(Box::new(Source::from(pair.into_inner()))),
            Rule::evaluation => Primary::Evaluation(Evaluation::from(pair.into_inner())),
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Evaluation {
    left: String,
    rights: Vec<EvaluationRight>,
}

impl Evaluation {
    fn eval(self, env: &mut Env) -> Type {
        let mut base = env.get_value(&self.left);

        for right in self.rights {
            use EvaluationRight::*;
            match right {
                Access(name) => {
                    if let Type::Block(s) = base {
                        let mut env = Env {
                            binds: s.binds,
                            evaluated: HashMap::new(),
                            parent: Some(Rc::new(RefCell::new(env.clone())))
                        };
                        base = env.get_value(&name);
                    }
                }
                _ => unreachable!()
            }
        }
        base
    }
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
enum EvaluationRight {
    Call(Additive),
    Access(String),
}

impl From<Pair<'_, Rule>> for EvaluationRight {
    fn from(pair: Pair<Rule>) -> Self {
        use EvaluationRight::*;
        match pair.as_rule() {
            Rule::calling => Call(Additive::from(pair.into_inner())),
            Rule::identify => Access(pair.as_str().to_string()),
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[test]
fn test_bind_and_access() {
    let ast = "hoge: {
  foo: 12
  bar: 23
  baz: foo + bar
}

  hoge.baz
";
    let source = Source::from(Parser::parse(Rule::source, ast).unwrap());
    assert!(source.eval() == Type::Number(35.0));

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
fn test_parsing_source() {
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
