// src/token.rs

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Begin,
    End,
    EndDot,
    If,
    Then,
    Else,
    Func,
    Return,
    Const,
    Import,
    From,
    Array,
    Class,
    New,
    Extends,
    Enum,
    Extern,
    Map,
    Contains,
    Record,
    Spawn,

    // Polimorfismo Real
    Override, // override

    Shl,
    Shr,
    Xor,
    BitAnd,
    BitOr,

    While,
    Do,
    For,
    To,
    In,

    Try,
    Catch,
    Throw,

    And,
    Or,
    Not,

    TypeInt,
    TypeFloat,
    TypeBool,
    TypeString,
    TypeChar,

    Ident(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    CharLiteral(char),
    BoolLiteral(bool),

    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    EqualEqual,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,

    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,

    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            kind,
            lexeme: lexeme.into(),
            line,
            column,
        }
    }
}

pub fn lookup_keyword(text: &str) -> Option<TokenKind> {
    match text {
        "begin" => Some(TokenKind::Begin),
        "end" => Some(TokenKind::End),
        "if" => Some(TokenKind::If),
        "then" => Some(TokenKind::Then),
        "else" => Some(TokenKind::Else),
        "func" => Some(TokenKind::Func),
        "return" => Some(TokenKind::Return),
        "const" => Some(TokenKind::Const),
        "import" => Some(TokenKind::Import),
        "from" => Some(TokenKind::From),
        "array" => Some(TokenKind::Array),
        "class" => Some(TokenKind::Class),
        "new" => Some(TokenKind::New),
        "extends" => Some(TokenKind::Extends),
        "enum" => Some(TokenKind::Enum),
        "extern" => Some(TokenKind::Extern),
        "map" => Some(TokenKind::Map),
        "contains" => Some(TokenKind::Contains),
        "record" => Some(TokenKind::Record),
        "spawn" => Some(TokenKind::Spawn),

        // Fase 15
        "override" => Some(TokenKind::Override),

        "shl" => Some(TokenKind::Shl),
        "shr" => Some(TokenKind::Shr),
        "xor" => Some(TokenKind::Xor),
        "bitand" => Some(TokenKind::BitAnd),
        "bitor" => Some(TokenKind::BitOr),

        "while" => Some(TokenKind::While),
        "do" => Some(TokenKind::Do),
        "for" => Some(TokenKind::For),
        "to" => Some(TokenKind::To),
        "in" => Some(TokenKind::In),

        "try" => Some(TokenKind::Try),
        "catch" => Some(TokenKind::Catch),
        "throw" => Some(TokenKind::Throw),

        "and" => Some(TokenKind::And),
        "or" => Some(TokenKind::Or),
        "not" => Some(TokenKind::Not),

        "int" => Some(TokenKind::TypeInt),
        "float" => Some(TokenKind::TypeFloat),
        "bool" => Some(TokenKind::TypeBool),
        "string" => Some(TokenKind::TypeString),
        "char" => Some(TokenKind::TypeChar),

        "true" => Some(TokenKind::BoolLiteral(true)),
        "false" => Some(TokenKind::BoolLiteral(false)),

        _ => None,
    }
}
