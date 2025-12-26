use crate::errors::LanguriaError;
use crate::errors::LanguriaResult;
use crate::lexer::Tok;
use crate::parser::AstNode::Root;
use logos::Lexer;
use rowan::Checkpoint;
use std::iter::Peekable;

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u16)]
pub enum AstNode {
    // values
    Num,
    //unary
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    // top level, keep for last in this list
    Root,
}

// plumbing for rowan
// https://github.com/rust-analyzer/rowan/blob/master/examples/s_expressions.rs
impl From<AstNode> for rowan::SyntaxKind {
    fn from(value: AstNode) -> Self {
        Self(value as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Languria {}

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
            Tok::Num(_) | Tok::Whitespace | Tok::__Test_Eof => 0,

            Tok::Plus | Tok::Minus => 0b_0001_0000,
            Tok::Star | Tok::Slash | Tok::Percent => 0b_0010_0000,
            Tok::Caret => 0b_1000_0000,
        }
    }
}

macro_rules! next {
    ($self:expr) => {{
        $self.skip_whitespace()?;
        let result = match $self.lexer.next() {
            Some(Ok(tok)) => Ok(tok),
            Some(Err(())) => Err(LanguriaError::InternalError(
                "No lexing rule matches the input".to_string(),
            )),
            None => Err(LanguriaError::SemanticUnexpectedEOF),
        }?;
        $self.skip_whitespace()?;
        result
    }};
}

macro_rules! peek {
    ($self:expr) => {{
        match $self.lexer.peek() {
            Some(Ok(tok)) => Ok(Some(*tok)),
            Some(Err(())) => Err(LanguriaError::InternalError(
                "No lexing rule matches the input".to_string(),
            )),
            None => Ok(None),
        }?
    }};
}

pub struct Parser<'src> {
    source: &'src str,
    lexer: Peekable<Lexer<'src, Tok<'src>>>,
    ast_builder: rowan::GreenNodeBuilder<'src>, // could be static?
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            lexer: Lexer::new(source).peekable(),
            ast_builder: Default::default(),
        }
    }

    pub fn parse(&mut self) -> LanguriaResult<()> {
        self.ast_builder.start_node(Root.into());
        while peek!(self).is_some() {
            self.parse_expr(0)?;
        }
        self.ast_builder.finish_node();
        Ok(())
    }

    fn parse_expr(&mut self, precedence: u8) -> LanguriaResult<()> {
        let checkpoint = self.ast_builder.checkpoint();
        self.parse_prefix()?;

        loop {
            let next_precedence = match peek!(self) {
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
        let tok = next!(self);
        match tok {
            Tok::Num(value) => self.add_leaf_node(AstNode::Num, value),
            _ => return Err(LanguriaError::UnexpectedToken(tok.info())),
        }
        Ok(())
    }

    fn add_leaf_node(&mut self, node: AstNode, value: &str) {
        self.ast_builder.start_node(node.into());
        self.ast_builder.token(node.into(), value);
        self.ast_builder.finish_node();
    }

    fn parse_infix(&mut self, checkpoint: Checkpoint, precedence: u8) -> LanguriaResult<()> {
        let tok = next!(self);
        match tok {
            Tok::Plus => self.add_binary_node(AstNode::Add, checkpoint, precedence)?,
            Tok::Minus => self.add_binary_node(AstNode::Subtract, checkpoint, precedence)?,
            Tok::Star => self.add_binary_node(AstNode::Multiply, checkpoint, precedence)?,
            Tok::Slash => self.add_binary_node(AstNode::Divide, checkpoint, precedence)?,
            Tok::Caret => self.add_binary_node(AstNode::Power, checkpoint, precedence)?,
            Tok::Percent => self.add_binary_node(AstNode::Modulo, checkpoint, precedence)?,
            _ => return Err(LanguriaError::UnexpectedToken(tok.info())),
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

    fn skip_whitespace(&mut self) -> LanguriaResult<()> {
        while let Some(Tok::Whitespace) = peek!(self) {
            self.lexer.next();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::SyntaxNode;

    fn print_ast(source: &str) {
        let mut parser = Parser::new(source);
        parser.parse().unwrap();
        let ast = parser.ast_builder.finish();
        let root: SyntaxNode<Languria> = SyntaxNode::new_root(ast);
        root.descendants().for_each(|node| println!("{:?}", node));
    }

    #[test]
    fn test_print_ast() {
        print_ast("1 + 2 * 3");
        // todo expect ast structure
    }
}
