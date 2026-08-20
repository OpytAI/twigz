//! Spanned surface syntax for the grammar language.

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Span {
    pub source: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub span: Span,
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn new(
        source: impl Into<String>,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span: Span {
                source: source.into(),
                start: 0,
                end: 0,
                line,
                column,
            },
            code: "compile".into(),
            message: message.into(),
        }
    }

    pub fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let mut parts = message.splitn(4, ':');
        let source = parts.next().unwrap_or("");
        let line = parts.next().and_then(|value| value.parse().ok());
        let column = parts.next().and_then(|value| value.parse().ok());
        let rest = parts.next();
        if let (Some(line), Some(column), Some(rest)) = (line, column, rest) {
            return Self::new(source, line, column, rest.trim_start());
        }
        Self::new(String::new(), 0, 0, message)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.span.source, self.span.line, self.span.column, self.message
        )
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleKind {
    Grammar,
    Family,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub kind: ModuleKind,
    pub name: String,
    pub version: String,
    pub start: Option<String>,
    pub uses: Vec<String>,
    pub declarations: Vec<Declaration>,
    pub comments: Vec<Comment>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Comment {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Associativity {
    Plain,
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(String),
    Pattern {
        value: String,
        flags: String,
    },
    Symbol(String),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Choice(Vec<Expr>),
    Sequence(Vec<Expr>),
    Optional(Box<Expr>),
    Repeat(Box<Expr>),
    Repeat1(Box<Expr>),
    Field {
        name: String,
        content: Box<Expr>,
    },
    Precedence {
        associativity: Associativity,
        value: i32,
        content: Box<Expr>,
    },
    AnyByte,
    Not(Box<Expr>),
    RepeatKept {
        literal: String,
        capture: String,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Semantic {
    pub kind: String,
    /// canonical role -> concrete field
    pub roles: Vec<(String, String)>,
    pub traits: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct OperatorRow {
    pub associativity: Associativity,
    pub precedence: i32,
    pub operators: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum MixedIndent {
    Error,
    TabIsNSpaces,
}

#[derive(Clone, Debug)]
pub enum Declaration {
    Rule {
        name: String,
        expression: Expr,
        open: bool,
        token: bool,
        semantic: Option<Semantic>,
        span: Span,
    },
    Extend {
        name: String,
        expression: Expr,
        span: Span,
    },
    Slot {
        name: String,
        span: Span,
    },
    Fill {
        name: String,
        expression: Expr,
        span: Span,
    },
    Fragment {
        name: String,
        parameters: Vec<String>,
        expression: Expr,
        span: Span,
    },
    Skip {
        expression: Expr,
        span: Span,
    },
    Externals {
        names: Vec<String>,
        span: Span,
    },
    Word {
        name: String,
        span: Span,
    },
    Conflict {
        names: Vec<String>,
        span: Span,
    },
    Mapping {
        concrete: String,
        semantic: Semantic,
        span: Span,
    },
    OperatorTable {
        name: String,
        operand: String,
        prefix: bool,
        rows: Vec<OperatorRow>,
        semantic: Option<Semantic>,
        span: Span,
    },
    Scan {
        name: String,
        expression: Expr,
        keep: Vec<String>,
        semantic: Option<Semantic>,
        span: Span,
    },
    ScanIndent {
        newline: String,
        indent: String,
        dedent: String,
        tab_width: u8,
        mixed: MixedIndent,
        span: Span,
    },
    ScanSlash {
        regex: String,
        division: String,
        span: Span,
    },
    ScanTemplate {
        open: String,
        close: String,
        chunk: Option<String>,
        span: Span,
    },
}
