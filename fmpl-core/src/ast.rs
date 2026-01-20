//! Abstract Syntax Tree for FMPL.

use crate::grammar::Grammar;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// A qualified name like `foo::bar::baz`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualifiedName {
    pub parts: Vec<SmolStr>,
}

impl QualifiedName {
    pub fn simple(name: SmolStr) -> Self {
        Self { parts: vec![name] }
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join("::"))
    }
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    Pipe,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// Visibility for object bindings.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Visibility {
    #[default]
    Private,
    Public,
    Protected,
}

/// A binding in an object definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub name: SmolStr,
    pub params: Vec<SmolStr>,
    pub has_params: bool,
    pub value: Expr,
    pub visibility: Visibility,
}

/// An object definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectDef {
    pub name: QualifiedName,
    pub params: Vec<SmolStr>,
    pub parents: Vec<QualifiedName>,
    pub bindings: Vec<Binding>,
    pub facets: Vec<FacetDef>,
}

/// A facet definition within an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FacetDef {
    pub name: SmolStr,
    pub members: Vec<SmolStr>,
    pub terminal: bool,
}

/// Pattern for match expressions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// Matches anything, binds to name.
    Var(SmolStr),
    /// Matches anything, discards.
    Wildcard,
    /// Matches literal int.
    Int(i64),
    /// Matches literal float.
    Float(f64),
    /// Matches literal string.
    String(SmolStr),
    /// Matches symbol.
    Symbol(SmolStr),
    /// Matches list with optional tail binding.
    List(Vec<Pattern>, Option<SmolStr>),
    /// Matches map with specific keys.
    Map(Vec<(SmolStr, Pattern)>),
    /// Constructor pattern.
    Constructor(SmolStr, Vec<Pattern>),
    /// Binds matched value to name.
    As(Box<Pattern>, SmolStr),
}

/// A match case (pattern => expression).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchCase {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

/// Let binding (possibly with destructuring).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LetBinding {
    Simple(SmolStr, Option<Box<Expr>>),
    Destructure(Pattern, Box<Expr>),
}

/// The core expression type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// String literal.
    String(SmolStr),
    /// Symbol literal (:foo).
    Symbol(SmolStr),
    /// Boolean literal.
    Bool(bool),
    /// Null literal.
    Null,
    /// Variable reference.
    Ident(SmolStr),
    /// Qualified name (foo::bar).
    Qualified(QualifiedName),
    /// Object tag (^foo).
    ObjTag(SmolStr),
    /// Function tag (@foo).
    FnTag(SmolStr),
    /// Self reference.
    Self_,
    /// Parent reference.
    Parent,
    /// Caller reference.
    Caller,
    /// User reference.
    User,
    /// Args reference.
    Args,

    /// List literal.
    List(Vec<Expr>),
    /// List with head | tail.
    ListCons(Box<Expr>, Box<Expr>),
    /// Map literal.
    Map(Vec<MapEntry>),

    /// Binary operation.
    Binary(Box<Expr>, BinOp, Box<Expr>),
    /// Unary operation.
    Unary(UnaryOp, Box<Expr>),

    /// Index access (expr[index]).
    Index(Box<Expr>, Box<Expr>),
    /// Slice access (expr[start..end]).
    Slice(Box<Expr>, Box<Expr>, Box<Expr>),

    /// Function call.
    Call(Box<Expr>, Vec<Arg>),
    /// Property access.
    PropAccess(Box<Expr>, SmolStr),
    /// Method call.
    MethodCall(Box<Expr>, SmolStr, Vec<Arg>),

    /// If expression.
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    /// While loop.
    While(Box<Expr>, Box<Expr>),
    /// Do-while loop.
    DoWhile(Box<Expr>, Box<Expr>),
    /// Return statement.
    Return(Option<Box<Expr>>),

    /// Lambda expression.
    Lambda(Vec<SmolStr>, Box<Expr>),
    /// Short lambda (\x expr).
    ShortLambda(SmolStr, Box<Expr>),
    /// Let expression.
    Let(Vec<LetBinding>, Box<Expr>),

    /// Sequence (block).
    Sequence(Vec<Expr>),

    /// Object definition.
    ObjectDef(ObjectDef),

    /// Match expression.
    Match(Box<Expr>, Vec<MatchCase>),

    /// Spawn object instance.
    Spawn(Box<Expr>, Vec<Arg>),
    /// Sync call ($expr).
    SyncCall(Box<Expr>),
    /// Async call (<- expr).
    AsyncCall(Box<Expr>),

    /// Try/catch expression.
    TryCatch {
        body: Box<Expr>,
        error_binding: SmolStr,
        catch_body: Box<Expr>,
    },

    /// Throw expression.
    Throw(Box<Expr>),

    /// Facet access (expr.as(:facet)).
    FacetAccess(Box<Expr>, SmolStr),

    /// Placeholder for partial application.
    Placeholder,

    /// Grammar application: expr @ grammar.rule
    /// The grammar can be a qualified name (static) or any expression (dynamic).
    GrammarApply {
        input: Box<Expr>,
        grammar: Box<Expr>,
        rule: SmolStr,
    },

    /// Anonymous grammar literal: grammar { rules }
    GrammarLiteral(Grammar),

    /// Grammar extension: base <: { rules }
    GrammarExtend { base: Box<Expr>, rules: Grammar },

    /// Stream literal: stream { expr }
    StreamLiteral(Box<Expr>),
}

/// Map entry (key: val or expr => expr).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapEntry {
    Symbol(SmolStr, Expr),
    Computed(Expr, Expr),
}

/// Function argument (may be placeholder for partial application).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Arg {
    Expr(Expr),
    Placeholder,
}
