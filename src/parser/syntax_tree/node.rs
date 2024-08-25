use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::ToPrimitive;
use rowan::{GreenNode, NodeOrToken, SyntaxKind};

pub struct tmp {
    kind: AstNodeKind,
    id: u64,
}

#[derive(FromPrimitive, ToPrimitive, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AstNodeKind {
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
}

impl From<AstNodeKind> for SyntaxKind {
    fn from(value: AstNodeKind) -> Self {
        SyntaxKind(value.to_u16().unwrap())
    }
}

impl From<NodeOrToken<GreenNode, GreenNode>> for AstNodeKind {
    fn from(value: NodeOrToken<GreenNode, GreenNode>) -> Self {
        value.into()
    }
}