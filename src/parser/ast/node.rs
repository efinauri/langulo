use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::ToPrimitive;
use rowan::{GreenNode, NodeOrToken, SyntaxKind};

#[derive(FromPrimitive, ToPrimitive, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AstNode {
    // leaf nodes
    Root,
    Int,
    Float,
    Bool,
    Str,
    Char,
    Comment,
    Whitespace,
    Identifier,

    // binary
    Add,
    Subtract,
    Multiply,
    Divide,
    LogicalAnd,
    LogicalOr,
    LogicalXor,

    // unary
    LogicalNot,
    Modulo,
    Grouping,
    Scope,
    Else,
    If,
    VarDecl,
    TypeAnnotation,
    TypeChar,
    TypeInt,
    TypeFloat,
    TypeBool,
    TypeStr,
    Table,
    TablePair,
    TypeTable,
    DefaultKey,
    FunctionDecl,
    PrincipalParam,
    ContourParam,
    FunctionAppl,
    ContourArgs,
    Lambda,
    TypeFn,
    ContourTypes,
    Option,
    TypeOption,
    Print,
}

impl From<AstNode> for SyntaxKind {
    fn from(value: AstNode) -> Self {
        SyntaxKind(value.to_u16().unwrap())
    }
}

impl From<NodeOrToken<GreenNode, GreenNode>> for AstNode {
    fn from(value: NodeOrToken<GreenNode, GreenNode>) -> Self {
        value.into()
    }
}