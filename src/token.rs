pub type AST = Statement;

pub type Bind = (String, Expression);

#[derive(Clone, Debug)]
pub struct Statement {
    pub definitions: Vec<Bind>,
    pub body: Expression,
}

#[derive(Clone, Debug)]
pub enum Expression {
    Comparison(Comparison),
    If {
        cond: Box<Expression>,
        cons: Box<Expression>,
        alt: Box<Expression>,
    },
}

#[derive(Clone, Debug)]
pub struct Comparison {
    pub left: Additive,
    pub rights: Vec<ComparisonRight>,
}

#[derive(Clone, Debug)]
pub enum ComparisonRight {
    Equal(Additive),
    NotEqual(Additive),
    GreaterThan(Additive),
    LessThan(Additive),
    NotGreaterThan(Additive),
    NotLessThan(Additive),
}

#[derive(Clone, Debug)]
pub struct Additive {
    pub left: Multitive,
    pub rights: Vec<AdditiveRight>,
}

#[derive(Clone, Debug)]
pub enum AdditiveRight {
    Add(Multitive),
    Sub(Multitive),
}

#[derive(Clone, Debug)]
pub struct Multitive {
    pub left: Operation,
    pub rights: Vec<MultitiveRight>,
}

#[derive(Clone, Debug)]
pub enum MultitiveRight {
    Mul(Operation),
    Div(Operation),
    Surplus(Operation),
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub left: Primary,
    pub rights: Vec<OperationRight>,
}

#[derive(Clone, Debug)]
pub enum OperationRight {
    Access(String),
    Call(Vec<Expression>),
    Index(Expression),
}

#[derive(Clone, Debug)]
pub enum Primary {
    Number(f64),
    String(String),
    Variable(String),
    Block(Box<Statement>),
    List(Vec<Expression>),
    Function(Vec<String>, Box<Expression>),
    Struct(Vec<Bind>),
    Null,
}
