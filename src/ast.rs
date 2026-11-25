// src/ast.rs

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    And,
    Or,
    Shl,
    Shr,
    Xor,
    BitAnd,
    BitOr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    StringLit(String),
    Boolean(bool),
    Variable(String),

    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        right: Box<Expr>,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    New {
        class_name: String,
        args: Vec<Expr>,
    },
    MemberAccess {
        object: Box<Expr>,
        member: String,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    ArrayLiteral(Vec<Expr>),
    NewArray {
        element_type: Type,
        size: Box<Expr>,
    },
    NewMap {
        key_type: Type,
        val_type: Type,
    },
    IndexAccess {
        array: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Located {
        source_file: String,
        line: usize,
        column: usize,
        statement: Box<Stmt>,
    },
    VarDecl {
        var_type: Type,
        names: Vec<String>,
    },
    ConstDecl {
        const_type: Option<Type>,
        name: String,
        value: Expr,
    },
    Assign {
        name: String,
        value: Expr,
    },
    MemberAssign {
        object: Box<Expr>,
        member: String,
        value: Expr,
    },
    IndexAssign {
        array: Box<Expr>,
        index: Box<Expr>,
        value: Expr,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    ForRange {
        var_name: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    ForIn {
        var_name: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    TryCatch {
        try_block: Vec<Stmt>,
        catch_var: String,
        catch_block: Vec<Stmt>,
    },
    Throw(Expr),
    Spawn {
        func_name: String,
    },
    Return(Option<Expr>),
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub is_override: bool, // <- Indica si es un método polimórfico
    pub line: usize,
    pub column: usize,
    pub source_file: String,
    pub params: Vec<(Type, String)>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternFuncDecl {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub source_file: String,
    pub params: Vec<(Type, String)>,
    pub return_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub field_type: Type,
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub type_line: usize,
    pub type_column: usize,
    pub source_file: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub source_file: String,
    pub parent: Option<String>,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<FunctionDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordDecl {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub source_file: String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub module_path: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub enums: Vec<EnumDecl>,
    pub records: Vec<RecordDecl>,
    pub extern_funcs: Vec<ExternFuncDecl>,
    pub classes: Vec<ClassDecl>,
    pub functions: Vec<FunctionDecl>,
    pub main_block: Vec<Stmt>,
}
