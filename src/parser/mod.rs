mod precedence;
pub mod syntax_tree;

use crate::errors::err::LanguloErr;
use crate::lexer::tok::Tok;
use crate::lexer::Lexer;
use crate::parser::syntax_tree::lang::LanguloSyntaxNode;
use crate::parser::syntax_tree::node::AstNode;
use rowan::Checkpoint;

pub type ASTBuilder = rowan::GreenNodeBuilder<'static>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    builder: ASTBuilder,
    semicolon_required: bool
}

// macro to avoid double mut borrow
macro_rules! next {
    ($self:expr) => {{
        let result = $self.lexer.next()?.ok_or_else(|| LanguloErr::semantic("Unexpected EOF"))?;
        result
    }};
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            lexer: Lexer::new(input),
            builder: ASTBuilder::new(),
            semicolon_required: true,
        }
    }

    pub fn to_ast(self) -> LanguloSyntaxNode {
        LanguloSyntaxNode::new_root(self.builder.finish())
    }

    fn new_leaf_node(&mut self, expr: AstNode, content: &str) -> Result<(), LanguloErr> {
        self.builder.start_node(expr.into());
        self.builder.token(expr.into(), content);
        self.builder.finish_node();
        Ok(())
    }

    fn new_binary_node(&mut self, kind: AstNode, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        self.builder.start_node_at(checkpoint, kind.into());
        self.parse_expr(precedence)?;
        self.builder.finish_node();
        Ok(())
    }

    fn new_prefix_unary_node(&mut self, kind: AstNode, tok: &Tok) -> Result<(), LanguloErr> {
        self.builder.start_node(kind.into());
        self.parse_expr(tok.precedence())?;
        self.builder.finish_node();
        Ok(())
    }

    fn new_postfix_unary_node(&mut self, kind: AstNode, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        self.builder.start_node_at(checkpoint, kind.into());
        self.parse_expr(precedence)?;
        self.builder.finish_node();
        Ok(())
    }

    pub fn parse(&mut self) -> Result<(), LanguloErr> {
        self.builder.start_node(AstNode::Root.into());
        while self.lexer.peek()?.is_some() {
            self.parse_expr(0)?;
            self.handle_semicolon()?;
        }
        self.builder.finish_node();
        Ok(())
    }

    pub fn parse_expr(&mut self, precedence: u8) -> Result<(), LanguloErr> {
        self.skip_trivia()?;
        let checkpoint = self.builder.checkpoint();

        self.parse_prefix()?;

        loop {
            self.skip_trivia()?;
            let tok_precedence = match self.lexer.peek()? {
                Some((tok, _)) => tok.precedence(),
                None => break,
            };
            if tok_precedence <= precedence { break; }

            self.parse_postfix(checkpoint, tok_precedence)?;
        }

        Ok(())
    }

    fn parse_prefix(&mut self) -> Result<(), LanguloErr> {
        self.skip_trivia()?;
        let (tok, content) = next!(self);

        match tok {
            Tok::Int => self.new_leaf_node(AstNode::Int, content)?,
            Tok::Float => self.new_leaf_node(AstNode::Float, content)?,
            Tok::Bool => self.new_leaf_node(AstNode::Bool, content)?,
            Tok::Char => self.new_leaf_node(AstNode::Char, content)?,
            Tok::Str => self.new_leaf_node(AstNode::Str, content)?,
            Tok::Not => self.new_prefix_unary_node(AstNode::LogicalNot, &tok)?,
            _ => return Err(LanguloErr::semantic(
                &*format!("Expected a literal or prefix operator, but found {}", content)
            ))
        }
        Ok(())
    }

    fn parse_postfix(&mut self, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        self.skip_trivia()?;
        let (tok, content) = next!(self);

        match tok {
            Tok::Plus => self.new_binary_node(AstNode::Add, checkpoint, precedence)?,
            Tok::Minus => self.new_binary_node(AstNode::Subtract, checkpoint, precedence)?,
            Tok::Star => self.new_binary_node(AstNode::Multiply, checkpoint, precedence)?,
            Tok::Slash => self.new_binary_node(AstNode::Divide, checkpoint, precedence)?,
            Tok::Modulo => self.new_binary_node(AstNode::Modulo, checkpoint, precedence)?,
            Tok::And => self.new_binary_node(AstNode::LogicalAnd, checkpoint, precedence)?,
            Tok::Or => self.new_binary_node(AstNode::LogicalOr, checkpoint, precedence)?,
            _ => return Err(LanguloErr::semantic(
                &*format!("Expected an infix or postfix operator, but found {}", content)
            ))
        }
        Ok(())
    }

    fn skip_trivia(&mut self) -> Result<(), LanguloErr> {
        while let Some((tok, content)) = self.lexer.peek()? {
            match tok {
                Tok::Whitespace => {
                    self.builder.token(AstNode::Whitespace.into(), content);
                    self.lexer.next()?;
                }
                Tok::Comment => {
                    self.builder.token(AstNode::Comment.into(), content);
                    self.lexer.next()?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn handle_semicolon(&mut self) -> Result<(), LanguloErr> {
        let next_is_semicolon = matches! { self.lexer.peek()?, Some((Tok::Semicolon, _)) };
        if next_is_semicolon {
            next!(self);
            return Ok(());
        };
        if self.semicolon_required {
            return Err(LanguloErr::semantic("Expected the expression to end"));
        }
        self.semicolon_required = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::syntax_tree::lang::LanguloSyntaxNode;

    pub fn to_simplified_string(node: &LanguloSyntaxNode) -> String {
        let children: Vec<String> = node.children().map(|c| to_simplified_string(&c)).collect();
        if node.kind() == AstNode::Root { return children.join("\n"); }

        if children.is_empty() {
            format!("{}", node.text()) // Assuming `text()` returns the string content for leaf nodes
        } else if children.len() == 1 {
            format!("({:?} {})", node.kind(), children[0])
        } else {
            format!("({} {:?} {})", children[0], node.kind(), children[1])
        }
    }

    fn expect_parser(input: &str, expected_ast_repr: &str) {
        let mut parser = Parser::new(input);
        parser.parse().expect("failed to parse");
        let node = parser.builder.finish();
        let syntax_node = LanguloSyntaxNode::new_root(node);
        assert_eq!(to_simplified_string(&syntax_node), expected_ast_repr.to_string())
    }

    #[test]
    fn arithmetic() {
        expect_parser(
            "1 + 2 * 3;",
            "(1 Add (2 Multiply 3))",
        )
    }

    #[test]
    fn logical() {
        expect_parser(
            "true and not false;",
            "(true LogicalAnd (LogicalNot false))",
        )
    }

    #[test]
    fn multiexpr_program() {
        expect_parser(
            "1+2*3; true and not false;",
            "(1 Add (2 Multiply 3))\n(true LogicalAnd (LogicalNot false))",
        )
    }
}