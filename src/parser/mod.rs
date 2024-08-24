mod tok_utils;

use crate::errors::err::LanguloErr;
use crate::lexer::tok::Tok;
use crate::lexer::Lexer;
use crate::syntax_tree::expr::Expr;
use crate::syntax_tree::lang::LanguloSyntaxNode;
use rowan::Checkpoint;

pub type ASTBuilder = rowan::GreenNodeBuilder<'static>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    builder: ASTBuilder,
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
        }
    }

    pub fn to_ast(self) -> LanguloSyntaxNode {
        LanguloSyntaxNode::new_root(self.builder.finish())
    }

    fn new_leaf_node(&mut self, expr: Expr, content: &str) -> Result<(), LanguloErr> {
        self.builder.start_node(expr.into());
        self.builder.token(expr.into(), content);
        self.builder.finish_node();
        Ok(())
    }

    fn new_binary_node(&mut self, content: &str, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        self.builder.start_node_at(checkpoint, Expr::Binary.into());
        self.builder.token(Expr::Binary.into(), content);
        self.parse_expr(precedence)?;
        self.builder.finish_node();
        Ok(())
    }

    fn new_unary_node(&mut self, content: &str, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        self.builder.start_node_at(checkpoint, Expr::Unary.into());
        self.builder.token(Expr::Unary.into(), content);
        self.parse_expr(precedence)?;
        self.builder.finish_node();
        Ok(())
    }

    pub fn parse(&mut self) -> Result<(), LanguloErr> {
        self.builder.start_node(Expr::Root.into());
        self.parse_expr(0)?;
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

    fn parse_postfix(&mut self, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        let (tok, content) = next!(self);

        match tok {
            Tok::Plus
            | Tok::Minus
            | Tok::Star
            | Tok::And
            | Tok::Or
            => self.new_binary_node(content, checkpoint, precedence)?,

            Tok::Not => {
                self.new_unary_node(content, checkpoint, precedence)?;
            }
            _ => return Err(LanguloErr::semantic(
                &*format!("Expected an infix or postfix operator, but found {}", content)
            ))
        }
        Ok(())
    }

    fn parse_prefix(&mut self) -> Result<(), LanguloErr> {
        self.skip_trivia()?;
        let (tok, content) = next!(self);

        if matches!(tok, Tok::Int) { self.new_leaf_node(Expr::Int, content) } else { Ok(()) }
    }

    fn skip_trivia(&mut self) -> Result<(), LanguloErr> {
        while let Some((tok, content)) = self.lexer.peek()? {
            match tok {
                Tok::Whitespace => {
                    self.builder.token(Expr::Whitespace.into(), content);
                    self.lexer.next()?;
                }
                Tok::Comment => {
                    self.builder.token(Expr::Comment.into(), content);
                    self.lexer.next()?;
                }
                _ => break,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_tree::lang::LanguloSyntaxNode;

    fn expect_parser(input: &str) {
        let mut parser = Parser::new(input);
        parser.parse().expect("failed to parse");
        let node = parser.builder.finish();
        let syntax_node = LanguloSyntaxNode::new_root(node);
        println!("{:#?}", syntax_node)
    }

    #[test]
    fn arithmetic() {
        expect_parser("1+2*3")
    }
}