#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Null,
    String(String),
    Identifier(Identifier),
    ObjectLiteral(Vec<ObjectProperty>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Member {
        object: Box<Expr>,
        property: String,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Assign {
        target: Identifier,
        value: Box<Expr>,
    },
    AssignMember {
        object: Box<Expr>,
        property: String,
        value: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProperty {
    pub key: String,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Let,
    Const,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableDeclaration {
    pub kind: BindingKind,
    pub name: Identifier,
    pub initializer: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDeclaration {
    pub name: Identifier,
    pub params: Vec<Identifier>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForInitializer {
    VariableDeclaration(VariableDeclaration),
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expr>,
    pub consequent: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VariableDeclaration(VariableDeclaration),
    FunctionDeclaration(FunctionDeclaration),
    Return(Option<Expr>),
    Expression(Expr),
    Block(Vec<Stmt>),
    If {
        condition: Expr,
        consequent: Box<Stmt>,
        alternate: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    For {
        initializer: Option<ForInitializer>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    Switch {
        discriminant: Expr,
        cases: Vec<SwitchCase>,
    },
    Throw(Expr),
    Try {
        try_block: Vec<Stmt>,
        catch_param: Option<Identifier>,
        catch_block: Option<Vec<Stmt>>,
        finally_block: Option<Vec<Stmt>>,
    },
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Script {
    pub statements: Vec<Stmt>,
}
