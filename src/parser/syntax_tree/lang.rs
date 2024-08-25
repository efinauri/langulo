use num_traits::{FromPrimitive, ToPrimitive};
use rowan::{Language, SyntaxKind, SyntaxNode};
use crate::parser::syntax_tree::node::AstNodeKind;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum Langulo {}

impl Language for Langulo {
    type Kind = AstNodeKind;

    fn kind_from_raw(raw: SyntaxKind) -> Self::Kind {
        Self::Kind::from_i16(raw.0 as i16).unwrap()
    }

    fn kind_to_raw(kind: Self::Kind) -> SyntaxKind {
        SyntaxKind(kind.to_u16().unwrap())
    }
}

pub type LanguloSyntaxNode = SyntaxNode<Langulo>;