use pest::Parser as PestParser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser as PestParser;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::iter::IntoIterator;
use std::fmt;
use std::fmt::Debug;

#[derive(Clone)]
struct Native {
    function: Rc<Fn(Type) -> Type>
}

impl std::cmp::PartialEq for Native {
    fn eq(&self, _: &Native) -> bool {
        false
    }
}

impl std::fmt::Debug for Native {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native")
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Boolean {
    True,
    False
}

#[derive(Debug, Clone, PartialEq)]
enum Type {
    Number(f64),
    String(String),
    Block(Source),
    Boolean(Boolean),
    Native(Native)
}

impl Type {
    fn get_prop(self, env: &mut Env, name: &str) -> Type {
        match self {
            Type::Block(s) => {
                let mut child = Env {
                    binds: s.binds,
                    evaluated: HashMap::new(),
                    parent: Some(Rc::new(RefCell::new(env.clone())))
                };
                child.get_value(name)
            }
            Type::String(s) => {
                match name {
                    "concat" => {
                        let s = s.clone();
                        let function = move |src: Type| {
                            if let Type::String(src) = src {
                                return Type::String(format!("{}{}", s, src));
                            }
                            panic!();
                        };
                        Type::Native(Native {
                            function: Rc::new(function),
                        })
                    }
                    _ => panic!()
                }
            }
            _ => unreachable!()
        }
    }

    fn call(self, mut args: Vec<Type>) -> Type {
        match self {
            Type::Block(s) => s.call(args),
            Type::Native(n) => {
                (n.function)(args.pop().unwrap())
            }
            _ => unreachable!()
        }
    }
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
        if let Some(evaluated) = self.evaluated.get(name) {
            return evaluated.clone();
        }

        if let Some(binded) = self.binds.remove(name) {
            let value = binded.eval(self);
            self.evaluated.insert(name.to_string(), value.clone());
            return value;
        }
        println!("{}", name);
        self.parent.as_ref().unwrap().borrow_mut().get_value(name)
    }
}

type Expression = Additive;

#[derive(Debug, Clone, PartialEq)]
struct Source {
    binds: HashMap<String, Expression>,
    expressions: Vec<Expression>
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

    fn call(mut self, args: Vec<Type>) -> Type {
        let mut evaluated = HashMap::new();
        for (v, p) in args.into_iter().zip(self.expressions.iter()) {
            evaluated.insert(p.clone().into(), v);
        }
        let mut env = Env {
            binds: self.binds,
            evaluated,
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
                    let expression = Expression::from(inner.next().unwrap().into_inner());
                    binds.insert(name.to_string(), expression);
                }
                Rule::additive => expressions.push(Expression::from(pair.into_inner())),
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

impl Into<String> for Additive {
    fn into(self) -> String {
        if let Primary::Evaluation(e) = self.left.left {
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
    String(String),
    Parenthesis(Box<Expression>),
    Block(Box<Source>),
    Evaluation(Evaluation),
    If(Box<Expression>, Box<Expression>, Box<Expression>)
}

impl Primary {
    fn eval(self, env: &mut Env) -> Type {
        use Primary::*;
        match self {
            Number(f) => Type::Number(f),
            String(s) => Type::String(s),
            Parenthesis(a) => a.eval(env),
            Block(s) => Type::Block(*s),
            Evaluation(e) => e.eval(env),
            If(cond, cons, alt) => {
                match cond.eval(env) {
                    Type::Boolean(Boolean::True) => cons.eval(env),
                    Type::Boolean(Boolean::False) => alt.eval(env),
                    _ => panic!(),
                }
            }
        }
    }
}

impl From<Pair<'_, Rule>> for Primary {
    fn from(pair: Pair<Rule>) -> Self {
        match pair.as_rule() {
            Rule::parenthesis => Primary::Parenthesis(Box::new(Expression::from(pair.into_inner().next().unwrap().into_inner()))),
            Rule::number => Primary::Number(pair.as_str().parse().unwrap()),
            Rule::string => Primary::String(pair.as_str().to_string()),
            Rule::block => Primary::Block(Box::new(Source::from(pair.into_inner()))),
            Rule::evaluation => Primary::Evaluation(Evaluation::from(pair.into_inner())),
            Rule::_if => {
                let mut inner = pair.into_inner();
                Primary::If(
                    Box::new(Expression::from(inner.next().unwrap().into_inner())),
                    Box::new(Expression::from(inner.next().unwrap().into_inner())),
                    Box::new(Expression::from(inner.next().unwrap().into_inner()))
                )
            }
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
                    base = base.get_prop(env, &name);
                }
                Call(arg) => {
                    base = base.call(vec![arg.eval(env)]);
                }
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
    Call(Expression),
    Access(String),
}

impl From<Pair<'_, Rule>> for EvaluationRight {
    fn from(pair: Pair<Rule>) -> Self {
        use EvaluationRight::*;
        println!("{:?}\n", pair);
        match pair.as_rule() {
            Rule::calling => Call(Expression::from(pair.into_inner().next().unwrap().into_inner())),
            Rule::identify => Access(pair.as_str().to_string()),
            _ => unreachable!("{:?}", pair)
        }
    }
}

#[test]
fn test_call() {
    let ast = "hoge: {
  fuga,
  fuga + 1
},

hoge(1)
";
    let pairs = Parser::parse(Rule::source, ast).unwrap();
    let source = Source::from(pairs);
    assert!(source.eval() == Type::Number(2.0));
}

#[test]
fn test_string_concat() {
    let ast = "hoge: \"hoge\",
hoge.concat(\"fuga\")";
    let source = Source::from(Parser::parse(Rule::source, ast).unwrap());
    assert!(source.eval() == Type::String("hogefuga".to_string()));
}

#[test]
fn test_bind_and_access() {
    let ast = "hoge: {
  foo: 12,
  bar: 23,
  baz: foo + bar
},

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
fn test_parsing_source_1() {
    let ast = "i";
    Parser::parse(Rule::source, ast).unwrap();
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
    let ast = "fizzbuzz: {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz \"fizz\" \"\",
  buzz: if is_buzz \"buzz\" \"\",

  fizz.concat(buzz)
},
range({min: 1, max: 100}).map({i: fizzbuzz})";
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}

#[test]
fn test_parsing_source_6() {
    let ast = "i(1)";
    Source::from(Parser::parse(Rule::source, ast).unwrap());
}
