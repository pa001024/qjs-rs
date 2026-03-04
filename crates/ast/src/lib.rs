#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub value: String,
    pub has_escape: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    BitAnd,
    BitOr,
    BitXor,
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    In,
    InstanceOf,
    LogicalAnd,
    LogicalOr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,
    Await,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateTarget {
    Identifier(Identifier),
    Member {
        object: Box<Expr>,
        property: String,
    },
    MemberComputed {
        object: Box<Expr>,
        property: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Null,
    String(StringLiteral),
    RegexLiteral {
        pattern: String,
        flags: String,
    },
    This,
    Identifier(Identifier),
    Function {
        name: Option<Identifier>,
        params: Vec<Identifier>,
        body: Vec<Stmt>,
    },
    ObjectLiteral(Vec<ObjectProperty>),
    ArrayLiteral(Vec<Expr>),
    Elision,
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Conditional {
        condition: Box<Expr>,
        consequent: Box<Expr>,
        alternate: Box<Expr>,
    },
    Sequence(Vec<Expr>),
    Member {
        object: Box<Expr>,
        property: String,
    },
    MemberComputed {
        object: Box<Expr>,
        property: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    New {
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
    AssignMemberComputed {
        object: Box<Expr>,
        property: Box<Expr>,
        value: Box<Expr>,
    },
    Update {
        target: UpdateTarget,
        increment: bool,
        prefix: bool,
    },
    AnnexBCallAssignmentTarget {
        target: Box<Expr>,
    },
    SpreadArgument(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectPropertyKey {
    Static(String),
    ProtoSetter,
    Computed(Box<Expr>),
    Spread,
    AccessorGet(String),
    AccessorSet(String),
    AccessorGetComputed(Box<Expr>),
    AccessorSetComputed(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProperty {
    pub key: ObjectPropertyKey,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Let,
    Const,
    Var,
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
    VariableDeclarations(Vec<VariableDeclaration>),
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expr>,
    pub consequent: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Empty,
    VariableDeclaration(VariableDeclaration),
    VariableDeclarations(Vec<VariableDeclaration>),
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
    With {
        object: Expr,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
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
    Labeled {
        label: Identifier,
        body: Box<Stmt>,
    },
    Break,
    BreakLabel(Identifier),
    Continue,
    ContinueLabel(Identifier),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Script {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImportBinding {
    pub imported: String,
    pub local: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImport {
    pub specifier: String,
    pub bindings: Vec<ModuleImportBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleExport {
    pub exported: String,
    pub local: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Module {
    pub imports: Vec<ModuleImport>,
    pub exports: Vec<ModuleExport>,
    pub body: Script,
}
