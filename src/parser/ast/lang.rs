use crate::parser::ast::node::AstNode;
use num_traits::{FromPrimitive, ToPrimitive};
use rowan::{Language, SyntaxKind, SyntaxNode, TextRange};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum Langulo {}

impl Language for Langulo {
    type Kind = AstNode;

    fn kind_from_raw(raw: SyntaxKind) -> Self::Kind {
        Self::Kind::from_i16(raw.0 as i16).unwrap()
    }

    fn kind_to_raw(kind: Self::Kind) -> SyntaxKind {
        SyntaxKind(kind.to_u16().unwrap())
    }
}

pub type LanguloSyntaxNode = SyntaxNode<Langulo>;
pub type NodeId = (AstNode, TextRange);

// not-so-pretty way to get a unique identifier for a node
pub trait LanguloSyntaxNodeExt {
    fn id(&self) -> NodeId;
}
impl LanguloSyntaxNodeExt for LanguloSyntaxNode {
    fn id(&self) -> NodeId {
        (self.kind(), self.text_range())
    }
}
