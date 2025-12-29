use crate::errors::LanguloError;
use crate::errors::LanguloResult;
use crate::lexer::Tok;
use crate::parser::AstNode::Root;
use logos::Lexer;
use rowan::{Checkpoint, GreenNodeBuilder, NodeOrToken, SyntaxNode};
use std::iter::Peekable;

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u16)]
pub enum AstNode {
    Root = u16::MAX,
    // values
    Num = 0,
    Bool,
    Literal,
    Grouping,
    // binary
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    And,
    Or,
    Xor,
    Eq,
    Neq,
    Leq,
    Geq,
    Lt,
    Gt,
    Assign,
    // unary prefix
    Not,
    Print,
}

// plumbing for rowan
// https://github.com/rust-analyzer/rowan/blob/master/examples/s_expressions.rs
impl From<AstNode> for rowan::SyntaxKind {
    fn from(value: AstNode) -> Self {
        Self(value as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Langulo {}
pub type LanguloSyntaxNode = SyntaxNode<Langulo>;

impl rowan::Language for Langulo {
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
            | Tok::Literal(_)
            | Tok::True(_)
            | Tok::False(_)
            | Tok::Whitespace(_)
            | Tok::BlockComment(_)
            | Tok::LineComment(_)
            | Tok::LParen
            | Tok::RParen
            | Tok::__Test_Eof => 0,

            Tok::Assign => 0b_0000_0001,

            Tok::And(_) | Tok::Or(_) | Tok::Not(_) | Tok::Xor(_) => 0b_0000_0100,
            Tok::Eq(_) | Tok::Neq(_) => 0b_0000_1000,
            Tok::Lt | Tok::Gt | Tok::Leq(_) | Tok::Geq(_) => 0b_0000_1100,

            Tok::Plus | Tok::Minus => 0b_0001_0000,
            Tok::Star | Tok::Slash | Tok::Percent => 0b_0010_0000,
            Tok::Caret => 0b_1000_0000,
            Tok::Dollar => 0b_1100_0000,
        }
    }
}

impl Tok<'_> {
    fn len(&self) -> usize {
        match self {
            Tok::Num(slice)
            | Tok::Literal(slice)
            | Tok::True(slice)
            | Tok::False(slice)
            | Tok::And(slice)
            | Tok::Or(slice)
            | Tok::Not(slice)
            | Tok::Xor(slice)
            | Tok::LineComment(slice)
            | Tok::BlockComment(slice)
            | Tok::Eq(slice)
            | Tok::Neq(slice)
            | Tok::Leq(slice)
            | Tok::Geq(slice)
            | Tok::Whitespace(slice) => slice.len(),
            Tok::Plus
            | Tok::Minus
            | Tok::Star
            | Tok::Slash
            | Tok::Percent
            | Tok::Caret
            | Tok::Lt
            | Tok::Gt
            | Tok::Dollar
            | Tok::RParen
            | Tok::Assign
            | Tok::LParen => 1,
            Tok::__Test_Eof => 0,
        }
    }
}

fn flatten_print_nodes(root: LanguloSyntaxNode) -> LanguloSyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    flatten_recursive(&root, &mut builder, false);
    SyntaxNode::new_root(builder.finish())
}

fn flatten_recursive(node: &LanguloSyntaxNode, builder: &mut GreenNodeBuilder, add_marker: bool) {
    if node.kind() == AstNode::Print {
        assert_eq!(node.children().count(), 1);
        if let Some(child) = node.first_child() {
            flatten_recursive(&child, builder, true);
        }
    } else {
        builder.start_node(node.kind().into());

        if add_marker {
            builder.token(AstNode::Print.into(), "");
        }

        for child in node.children_with_tokens() {
            match child {
                NodeOrToken::Node(n) => flatten_recursive(&n, builder, false),
                NodeOrToken::Token(t) => builder.token(t.kind().into(), t.text()),
            }
        }

        builder.finish_node();
    }
}

pub fn has_print_marker(node: &LanguloSyntaxNode) -> bool {
    use rowan::NodeOrToken;
    node.children_with_tokens().any(|child| {
        matches!(child, NodeOrToken::Token(tok) if tok.kind() == AstNode::Print)
    })
}

pub struct Parser<'src> {
    source: &'src str,
    lexer: Peekable<Lexer<'src, Tok<'src>>>,
    ast_builder: rowan::GreenNodeBuilder<'src>,
    current_offset: usize,
}

pub fn parse(source: &str) -> LanguloResult<LanguloSyntaxNode> {
    let mut parser = Parser::new(source);
    parser.parse_root()?;
    let ast = parser.ast_builder.finish();
    let raw_tree = SyntaxNode::new_root(ast);
    Ok(flatten_print_nodes(raw_tree))
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

    fn next_token(&mut self) -> LanguloResult<Tok<'src>> {
        self.skip_trivia()?;
        match self.lexer.next() {
            Some(Ok(tok)) => {
                self.current_offset += tok.len();
                Ok(tok)
            }
            Some(Err(())) => Err(LanguloError::LexerError {
                src: self.source.into(),
                span: (self.current_offset, 1).into(),
            }),
            None => Err(LanguloError::UnexpectedEOF {
                src: self.source.into(),
                span: (self.current_offset, 1).into(),
            }),
        }
    }

    fn peek_token(&mut self) -> LanguloResult<Option<Tok<'src>>> {
        self.skip_trivia()?;
        match self.lexer.peek() {
            Some(Ok(tok)) => Ok(Some(*tok)),
            Some(Err(())) => Err(LanguloError::LexerError {
                src: self.source.into(),
                span: (self.current_offset, 1).into(),
            }),
            None => Ok(None),
        }
    }

    fn parse_root(&mut self) -> LanguloResult<()> {
        self.ast_builder.start_node(Root.into());
        while self.peek_token()?.is_some() {
            self.parse_expr(0)?;
        }
        self.ast_builder.finish_node();
        Ok(())
    }

    fn parse_expr(&mut self, precedence: u8) -> LanguloResult<()> {
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
            self.parse_infix(checkpoint, next_precedence, false)?;
        }
        Ok(())
    }

    fn parse_prefix(&mut self) -> LanguloResult<()> {
        let tok = self.next_token()?;
        match tok {
            Tok::Num(value) => self.add_leaf_node(AstNode::Num, value),
            Tok::Literal(value) => self.add_leaf_node(AstNode::Literal, value),
            Tok::True(value) | Tok::False(value) => self.add_leaf_node(AstNode::Bool, value),
            Tok::LParen => {
                self.ast_builder.start_node(AstNode::Grouping.into());
                self.parse_expr(0)?;
                self.require_specific_tok(Tok::RParen)?;
                self.ast_builder.finish_node();
            }
            Tok::Not(_) => self.add_unary_node_prefix(AstNode::Not, tok.precedence())?,
            Tok::Dollar => self.add_unary_node_prefix(AstNode::Print, tok.precedence())?,
            _ => {
                return Err(LanguloError::UnexpectedToken {
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

    fn parse_infix(&mut self, checkpoint: Checkpoint, precedence: u8, cond: bool) -> LanguloResult<()> {
        let tok = self.next_token()?;
        match tok {
            Tok::Plus => self.add_binary_node(AstNode::Add, checkpoint, precedence)?,
            Tok::Minus => self.add_binary_node(AstNode::Subtract, checkpoint, precedence)?,
            Tok::Star => self.add_binary_node(AstNode::Multiply, checkpoint, precedence)?,
            Tok::Slash => self.add_binary_node(AstNode::Divide, checkpoint, precedence)?,
            Tok::Caret => self.add_binary_node(AstNode::Power, checkpoint, precedence)?,
            Tok::Percent => self.add_binary_node(AstNode::Modulo, checkpoint, precedence)?,
            Tok::And(_) => self.add_binary_node(AstNode::And, checkpoint, precedence)?,
            Tok::Or(_) => self.add_binary_node(AstNode::Or, checkpoint, precedence)?,
            Tok::Xor(_) => self.add_binary_node(AstNode::Xor, checkpoint, precedence)?,
            Tok::Eq(_) => self.add_binary_node(AstNode::Eq, checkpoint, precedence)?,
            Tok::Neq(_) => self.add_binary_node(AstNode::Neq, checkpoint, precedence)?,
            Tok::Gt => self.add_binary_node(AstNode::Gt, checkpoint, precedence)?,
            Tok::Lt => self.add_binary_node(AstNode::Lt, checkpoint, precedence)?,
            Tok::Geq(_) => self.add_binary_node(AstNode::Geq, checkpoint, precedence)?,
            Tok::Leq(_) => self.add_binary_node(AstNode::Leq, checkpoint, precedence)?,
            Tok::Assign => self.add_binary_node(AstNode::Assign, checkpoint, precedence)?,
            Tok::Dollar => {
                if !cond {
                    // print operand can also act on infix operands themselves
                    self.ast_builder.start_node_at(checkpoint, AstNode::Print.into());
                    let next_precedence = match self.peek_token()? {
                        Some(tok) => tok.precedence(),
                        None => return Err(LanguloError::UnexpectedToken {
                            token: "end of file".into(),
                            src: self.source.into(),
                            span: (self.current_offset, 1).into(),
                        }),
                    };
                    self.parse_infix(checkpoint, next_precedence, true)?;
                    self.ast_builder.finish_node();
                }
            }
            _ => {
                return Err(LanguloError::UnexpectedToken {
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
    ) -> LanguloResult<()> {
        self.ast_builder.start_node_at(checkpoint, node.into());
        self.parse_expr(precedence)?;
        self.ast_builder.finish_node();
        Ok(())
    }

    fn add_unary_node_prefix(&mut self, node: AstNode, precedence: u8) -> LanguloResult<()> {
        self.ast_builder.start_node(node.into());
        self.parse_expr(precedence)?;
        self.ast_builder.finish_node();
        Ok(())
    }

    fn skip_trivia(&mut self) -> LanguloResult<()> {
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

    fn require_specific_tok(&mut self, required_tok: Tok) -> LanguloResult<()> {
        if let Some(found_tok) = self.peek_token()? {
            return if found_tok == required_tok {
                let _ = self.next_token()?;
                Ok(())
            } else {
                Err(LanguloError::ExpectedTokenAbsent {
                    expected: required_tok.info(),
                    found: found_tok.info(),
                    src: self.source.into(),
                    span: (self.current_offset, found_tok.len()).into(),
                })
            };
        }
        Err(LanguloError::ExpectedTokenAbsent {
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

    #[derive(Default)]
    struct AstExpectation<'a> {
        source: &'a str,
        nodes: &'a [AstNode],
        children: &'a [&'a [usize]],
        print_markers: &'a [usize],
    }

    fn expect_ast(exp: AstExpectation) {
        let root = parse(exp.source).unwrap();
        let nodes: Vec<_> = root.descendants().collect();
        let actual_nodes: Vec<_> = nodes.iter().map(|node| node.kind()).collect();

        assert_eq!(
            actual_nodes, exp.nodes,
            "AST node mismatch for '{}'", exp.source
        );

        for assertion in exp.children {
            if assertion.is_empty() {
                continue;
            }
            let parent_idx = assertion[0];
            let expected_children = &assertion[1..];
            let actual_children: Vec<_> = nodes[parent_idx]
                .children()
                .map(|child| nodes.iter().position(|n| n == &child).unwrap())
                .collect();
            assert_eq!(
                actual_children,
                expected_children,
                "Node at index {} ({:?}) has incorrect children",
                parent_idx,
                nodes[parent_idx].kind()
            );
        }

        for (idx, node) in nodes.iter().enumerate() {
            let should_have_marker = exp.print_markers.contains(&idx);
            let has_marker = has_print_marker(node);
            assert_eq!(
                has_marker,
                should_have_marker,
                "Node at index {} ({:?}): expected print_marker={}, got {}",
                idx,
                node.kind(),
                should_have_marker,
                has_marker
            );
        }
    }

    #[test]
    fn test_arithmetic() {
        expect_ast(AstExpectation {
            source: "1 + 2 * 3",
            nodes: &[Root, Add, Num, Multiply, Num, Num],
            ..Default::default()
        });

        expect_ast(AstExpectation {
            source: "2 ^ 3 ^ 2",
            nodes: &[Root, Power, Power, Num, Num, Num],
            ..Default::default()
        });

        expect_ast(AstExpectation {
            source: "1 + 2 * 3 - 4 / 2",
            nodes: &[Root, Subtract, Add, Num, Multiply, Num, Num, Divide, Num, Num],
            children: &[&[1, 2, 7], &[2, 3, 4], &[4, 5, 6], &[7, 8, 9]],
            ..Default::default()
        });
    }

    #[test]
    fn test_comments() {
        expect_ast(AstExpectation {
            source: "1 //- ignored -// + 2 * 3 //also ignored",
            nodes: &[Root, Add, Num, Multiply, Num, Num],
            ..Default::default()
        });
    }

    #[test]
    fn test_grouping() {
        expect_ast(AstExpectation {
            source: "2 * (3 - 1)",
            nodes: &[Root, Multiply, Num, Grouping, Subtract, Num, Num],
            children: &[&[1, 2, 3], &[3, 4], &[4, 5, 6]],
            ..Default::default()
        });
    }

    #[test]
    fn test_booleans() {
        expect_ast(AstExpectation {
            source: "not true and false xor true",
            nodes: &[Root, Xor, And, Not, Bool, Bool, Bool],
            children: &[&[1, 2, 6], &[2, 3, 5], &[3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_assign() {
        expect_ast(AstExpectation {
            source: "x = 5",
            nodes: &[Root, Assign, Literal, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_print_prefix_simple() {
        expect_ast(AstExpectation {
            source: "$3",
            nodes: &[Root, Num],
            print_markers: &[1],
            ..Default::default()
        });
    }

    #[test]
    fn test_print_prefix_in_expression() {
        expect_ast(AstExpectation {
            source: "1 + $2 + 3",
            nodes: &[Root, Add, Add, Num, Num, Num],
            print_markers: &[4],
            ..Default::default()
        });
    }

    #[test]
    fn test_print_on_grouped_assignment() {
        expect_ast(AstExpectation {
            source: "$(x = 3)",
            nodes: &[Root, Grouping, Assign, Literal, Num],
            print_markers: &[1],
            ..Default::default()
        });
    }

    #[test]
    fn test_print_on_rhs_of_assignment() {
        expect_ast(AstExpectation {
            source: "x = $3",
            nodes: &[Root, Assign, Literal, Num],
            print_markers: &[3],
            ..Default::default()
        });
    }

    #[test]
    fn test_nested_print() {
        expect_ast(AstExpectation {
            source: "$$3",
            nodes: &[Root, Num],
            print_markers: &[1],
            ..Default::default()
        });
    }

    #[test]
    fn test_print_on_other_operands() {
        expect_ast(AstExpectation {
            source: "x $= 1",
            nodes: &[Root, Assign, Literal, Num],
            print_markers: &[1],
            ..Default::default()
        });
    }
}
