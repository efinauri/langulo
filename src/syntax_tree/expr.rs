use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::ToPrimitive;
use rowan::{GreenNode, NodeOrToken, SyntaxKind};

#[derive(FromPrimitive, ToPrimitive, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Expr {
    Root,
    Identifier,
    Literal,
    Whitespace,
    Binary,
    Unary,
    Int,
    Comment,
}

impl From<Expr> for SyntaxKind {
    fn from(value: Expr) -> Self {
        SyntaxKind(value.to_u16().unwrap())
    }
}

impl From<NodeOrToken<GreenNode, GreenNode>> for Expr {
    fn from(value: NodeOrToken<GreenNode, GreenNode>) -> Self {
        value.into()
    }
}