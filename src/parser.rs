use std::char::MAX;
use crate::errors::LanguriaError;
use crate::errors::LanguriaResult;
use crate::lexer::Tok;
use crate::parser::AstNode::Root;
use logos::{Lexer, Source};
use rowan::{Checkpoint, SyntaxNode};
use std::iter::Peekable;

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u16)]
pub enum AstNode {
    Root = u16::MAX,
    // values
    Num = 0,
    Grouping,
    //unary
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
}

// plumbing for rowan
// https://github.com/rust-analyzer/rowan/blob/master/examples/s_expressions.rs
impl From<AstNode> for rowan::SyntaxKind {
    fn from(value: AstNode) -> Self {
        Self(value as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Languria {}
pub type LanguriaSyntaxNode = SyntaxNode<Languria>;

impl rowan::Language for Languria {
    type Kind = AstNode;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        assert!(raw.0 <= Root as u16);
        unsafe { std::mem::transmute::<u16, AstNode>(raw.0) }
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}
// end of plumbing

impl Tok<'_> {
    fn precedence(&self) -> u8 {
        match self {
            Tok::Num(_)
            | Tok::Whitespace(_)
            | Tok::BlockComment(_)
            | Tok::LineComment(_)
            | Tok::LParen
            | Tok::RParen
            | Tok::__Test_Eof => 0,

            Tok::Plus | Tok::Minus => 0b_0001_0000,
            Tok::Star | Tok::Slash | Tok::Percent => 0b_0010_0000,
            Tok::Caret => 0b_1000_0000,
        }
    }
}

impl Tok<'_> {
    fn len(&self) -> usize {
        match self {
            Tok::Num(slice)
            | Tok::LineComment(slice)
            | Tok::BlockComment(slice)
            | Tok::Whitespace(slice) => slice.len(),
            Tok::Plus
            | Tok::Minus
            | Tok::Star
            | Tok::Slash
            | Tok::Percent
            | Tok::Caret
            | Tok::RParen
            | Tok::LParen => 1,
            Tok::__Test_Eof => 0,
        }
    }
}

pub struct Parser<'src> {
    source: &'src str,
    lexer: Peekable<Lexer<'src, Tok<'src>>>,
    ast_builder: rowan::GreenNodeBuilder<'src>,
    current_offset: usize,
}

pub fn parse(source: &str) -> LanguriaResult<LanguriaSyntaxNode> {
    let mut parser = Parser::new(source);
    parser.parse_root()?;
    let ast = parser.ast_builder.finish();
    Ok(SyntaxNode::new_root(ast))
}

impl<'src> Parser<'src> {
    fn new(source: &'src str) -> Self {
        Self {
            source,
            lexer: Lexer::new(source).peekable(),
            ast_builder: Default::default(),
            current_offset: 0,
        }
    }

    fn next_token(&mut self) -> LanguriaResult<Tok<'src>> {
        self.skip_trivia()?;
        match self.lexer.next() {
            Some(Ok(tok)) => {
                self.current_offset += tok.len();
                Ok(tok)
            }
            Some(Err(())) => Err(LanguriaError::LexerError {
                src: self.source.into(),
                span: (self.current_offset, 1).into(),
            }),
            None => Err(LanguriaError::UnexpectedEOF {
                src: self.source.into(),
                span: (self.current_offset, 1).into(),
            }),
        }
    }

    fn peek_token(&mut self) -> LanguriaResult<Option<Tok<'src>>> {
        self.skip_trivia()?;
        match self.lexer.peek() {
            Some(Ok(tok)) => Ok(Some(*tok)),
            Some(Err(())) => Err(LanguriaError::LexerError {
                src: self.source.into(),
                span: (self.current_offset, 1).into(),
            }),
            None => Ok(None),
        }
    }

    fn parse_root(&mut self) -> LanguriaResult<()> {
        self.ast_builder.start_node(Root.into());
        while self.peek_token()?.is_some() {
            self.parse_expr(0)?;
        }
        self.ast_builder.finish_node();
        Ok(())
    }

    fn parse_expr(&mut self, precedence: u8) -> LanguriaResult<()> {
        let checkpoint = self.ast_builder.checkpoint();
        self.parse_prefix()?;

        loop {
            let next_precedence = match self.peek_token()? {
                Some(tok) => tok.precedence(),
                None => break,
            };
            if next_precedence <= precedence {
                break;
            }
            self.parse_infix(checkpoint, next_precedence)?;
        }
        Ok(())
    }

    fn parse_prefix(&mut self) -> LanguriaResult<()> {
        let tok = self.next_token()?;
        match tok {
            Tok::Num(value) => self.add_leaf_node(AstNode::Num, value),
            Tok::LParen => {
                self.ast_builder.start_node(AstNode::Grouping.into());
                self.parse_expr(0)?;
                self.require_specific_tok(Tok::RParen)?;
                self.ast_builder.finish_node();
            }
            _ => {
                return Err(LanguriaError::UnexpectedToken {
                    token: tok.info(),
                    src: self.source.into(),
                    span: (self.current_offset, tok.len()).into(),
                });
            }
        }
        Ok(())
    }

    fn add_leaf_node(&mut self, node: AstNode, value: &str) {
        self.ast_builder.start_node(node.into());
        self.ast_builder.token(node.into(), value);
        self.ast_builder.finish_node();
    }

    fn parse_infix(&mut self, checkpoint: Checkpoint, precedence: u8) -> LanguriaResult<()> {
        let tok = self.next_token()?;
        match tok {
            Tok::Plus => self.add_binary_node(AstNode::Add, checkpoint, precedence)?,
            Tok::Minus => self.add_binary_node(AstNode::Subtract, checkpoint, precedence)?,
            Tok::Star => self.add_binary_node(AstNode::Multiply, checkpoint, precedence)?,
            Tok::Slash => self.add_binary_node(AstNode::Divide, checkpoint, precedence)?,
            Tok::Caret => self.add_binary_node(AstNode::Power, checkpoint, precedence)?,
            Tok::Percent => self.add_binary_node(AstNode::Modulo, checkpoint, precedence)?,
            _ => {
                return Err(LanguriaError::UnexpectedToken {
                    token: tok.info(),
                    src: self.source.into(),
                    span: (self.current_offset, tok.len()).into(),
                });
            }
        }
        Ok(())
    }

    fn add_binary_node(
        &mut self,
        node: AstNode,
        checkpoint: Checkpoint,
        precedence: u8,
    ) -> LanguriaResult<()> {
        self.ast_builder.start_node_at(checkpoint, node.into());
        self.parse_expr(precedence)?;
        self.ast_builder.finish_node();
        Ok(())
    }

    fn skip_trivia(&mut self) -> LanguriaResult<()> {
        while let Some(Ok(tok)) = self.lexer.peek() {
            match tok {
                Tok::Whitespace(slice) | Tok::BlockComment(slice) | Tok::LineComment(slice) => {
                    self.current_offset += slice.len();
                    self.lexer.next();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn require_specific_tok(&mut self, required_tok: Tok) -> LanguriaResult<()> {
        if let Some(found_tok) = self.peek_token()? {
            return if found_tok == required_tok {
                let _ = self.next_token()?;
                Ok(())
            } else {
                Err(LanguriaError::ExpectedTokenAbsent {
                    expected: required_tok.info(),
                    found: found_tok.info(),
                    src: self.source.into(),
                    span: (self.current_offset, found_tok.len()).into(),
                })
            };
        }
        Err(LanguriaError::ExpectedTokenAbsent {
            expected: required_tok.info(),
            found: "end of file".into(),
            src: self.source.into(),
            span: (self.current_offset, 1).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::AstNode::*;

    fn print_ast(source: &str) {
        let root = parse(source).unwrap();
        root.descendants().for_each(|node| println!("{:?}", node));
    }

    fn expect_ast(source: &str, expected_nodes_tree_lexicographic_order: &[AstNode]) {
        let root = parse(source).unwrap();
        let actual_nodes: Vec<_> = root.descendants().map(|node| node.kind()).collect();
        assert_eq!(actual_nodes, expected_nodes_tree_lexicographic_order);
    }

    #[test]
    fn test_arithmetic() {
        print_ast("1 + 2 * 3");
        expect_ast("1 + 2 * 3", &[Root, Add, Num, Multiply, Num, Num]);
        expect_ast("2 ^ 3 ^ 2", &[Root, Power, Power, Num, Num, Num]);
        expect_ast(
            "1 + 2 * 3 - 4 / 2",
            &[
                Root, Subtract, Add, Num, Multiply, Num, Num, Divide, Num, Num,
            ],
        );
    }

    #[test]
    fn test_comments() {
        expect_ast(
            "1 //- ignored -// + 2 * 3 //also ignored",
            &[Root, Add, Num, Multiply, Num, Num],
        );
    }

    #[test]
    fn test_grouping() {
        expect_ast(
            "2 * (3 - 1)",
            &[Root, Multiply, Num, Grouping, Subtract, Num, Num]
        )
    }
}
