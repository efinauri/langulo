use crate::errors::LanguloError;
use crate::errors::LanguloResult;
use crate::lexer::Tok;
use crate::parser::AstNode::Root;
use logos::{Lexer, Source};
use miette::SourceSpan;
use rowan::{Checkpoint, GreenNodeBuilder, NodeOrToken, SyntaxNode};
use std::cmp::{max, min};
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
    Ask,
    // function
    FunctionDecl,
    FunctionParams,
    FunctionBody,
    PrefixFnCall,
    PostfixFnCall,
    CallArgs,
    Block,
    Return,
    StringLit,
    StringPart,
    InterpolationPart,
    SomeOption,
    NoOption,
    If,
    Else,
    ListLit,
    SetLit,
    Del,
    MapIndex,
    Range,
    IterVar,
    MapLit,
    MapEntry,
    Iter,
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
            | Tok::StringLitSingle(_)
            | Tok::StringLitDouble(_)
            | Tok::TripleDoubleQuote(_)
            | Tok::TripleSingleQuote(_)
            | Tok::Literal(_)
            | Tok::True(_)
            | Tok::False(_)
            | Tok::Whitespace(_)
            | Tok::LineContinuation(_)
            | Tok::BlockComment(_)
            | Tok::LineComment(_)
            | Tok::RParen
            | Tok::Comma
            | Tok::LBrace
            | Tok::RBrace
            | Tok::Pipe
            | Tok::Return(_)
            | Tok::Newline(_)
            | Tok::TrailingBackslash(_)
            | Tok::If(_)
            | Tok::QuestionMark
            | Tok::Colon
            | Tok::Ask(_)
            | Tok::Not(_)
            | Tok::Del(_)
            | Tok::Set(_)
            | Tok::List(_)
            | Tok::Key(_)
            | Tok::Value(_)
            | Tok::Index(_)
            | Tok::At
            | Tok::RBracket
            | Tok::__Test_Eof => 0,

            Tok::Assign => 0b_0000_0001,

            Tok::Else(_) | Tok::Iter(_) => 0b_0000_0010,

            Tok::DotDot(_) => 0b_0000_0011,

            Tok::And(_) | Tok::Or(_) | Tok::Xor(_) => 0b_0000_0100,
            Tok::Eq(_) | Tok::Neq(_) => 0b_0000_1000,
            Tok::Lt | Tok::Gt | Tok::Leq(_) | Tok::Geq(_) => 0b_0000_1100,

            Tok::Plus | Tok::Minus => 0b_0001_0000,
            Tok::Star | Tok::Slash | Tok::Percent => 0b_0010_0000,
            Tok::Caret => 0b_1000_0000,
            Tok::Dollar => 0b_1100_0000,
            Tok::Dot => 0b_1110_0000,
            Tok::LParen | Tok::LBracket | Tok::ExclamationMark => 0b_1111_0000, // for fn calls
        }
    }
}

/// some post-processing after the initial AST construction.
///
/// print nodes go from being the parent node of the thing they're printing to being a token payload.
///
/// this makes compilation easier, since you know right away what node you're dealing with during AST transversal,
/// and to check whether the node needs printing you [check its token payload](has_print_marker)
fn flatten_print_nodes(root: LanguloSyntaxNode) -> LanguloSyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    __recursive(&root, &mut builder, false);
    return SyntaxNode::new_root(builder.finish());

    fn __recursive(node: &LanguloSyntaxNode, builder: &mut GreenNodeBuilder, add_marker: bool) {
        if node.kind() == AstNode::Print {
            assert_eq!(node.children().count(), 1);
            let child = node.first_child().unwrap();
            __recursive(&child, builder, true);
        } else {
            // build the ast as is, optionally adding a print marker token
            builder.start_node(node.kind().into());

            if add_marker {
                builder.token(AstNode::Print.into(), "");
            }

            for child in node.children_with_tokens() {
                match child {
                    NodeOrToken::Node(n) => __recursive(&n, builder, false),
                    NodeOrToken::Token(t) => builder.token(t.kind().into(), t.text()),
                }
            }

            builder.finish_node();
        }
    }
}

pub fn has_print_marker(node: &LanguloSyntaxNode) -> bool {
    node.children_with_tokens()
        .any(|child| matches!(child, NodeOrToken::Token(tok) if tok.kind() == AstNode::Print))
}

/// implemented with pratt parsing
pub struct Parser<'src> {
    source: &'src str,
    lexer: Peekable<Lexer<'src, Tok<'src>>>,
    ast_builder: GreenNodeBuilder<'src>,
    /// with respect to `self.source`
    current_offset: usize,
    is_in_fn_body: bool,
    is_in_iter_body: bool,
}

pub fn parse(source: &str) -> LanguloResult<LanguloSyntaxNode> {
    let mut parser = Parser::new(source);
    parser.parse_root()?;
    let ast = parser.ast_builder.finish();
    // post processing
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
            is_in_fn_body: false,
            is_in_iter_body: false,
        }
    }

    /// small utility method to instantiate errors so that the token is marked as red
    fn span_highlighting_token(&self, other: &Tok<'src>) -> SourceSpan {
        (self.current_offset - other.len(), other.len()).into()
    }

    /// skips trivia
    fn next_meaningful_token(&mut self) -> LanguloResult<Tok<'src>> {
        self.skip_trivia()?;
        match self.lexer.next() {
            Some(Ok(tok)) => {
                self.current_offset += tok.len();
                if self.current_offset > self.source.len() {
                    return Err(LanguloError::InternalError {
                        _message: format!("offset went outside of source code because of tok {:?}", tok),
                    })
                }
                Ok(tok)
            }
            Some(Err(())) => Err(LanguloError::LexerError {
                _src: self.source.into(),
                _span: (self.current_offset, 0).into(),
            }),
            None => Err(LanguloError::UnexpectedEOF {
                _src: self.source.into(),
                _span: (max(0, self.current_offset - 1), 0).into(),
            }),
        }
    }

    /// skips trivia
    fn peek_meaningful_token(&mut self) -> LanguloResult<Option<Tok<'src>>> {
        self.skip_trivia()?;
        match self.lexer.peek() {
            Some(Ok(tok)) => Ok(Some(*tok)),
            Some(Err(())) => Err(LanguloError::LexerError {
                _src: self.source.into(),
                _span: (self.current_offset, 1).into(),
            }),
            None => Ok(None),
        }
    }

    fn peek_meaningful_token_or_eof_err(&mut self) -> LanguloResult<Tok<'src>> {
        self.peek_meaningful_token()?
            .ok_or(LanguloError::UnexpectedEOF {
                _src: self.source.into(),
                _span: (max(0, self.current_offset - 1), 0).into(),
            })
    }

    fn parse_root(&mut self) -> LanguloResult<()> {
        self.ast_builder.start_node(Root.into());
        self.skip_newlines()?;
        while self.peek_meaningful_token()?.is_some() {
            self.parse_expr(0)?;
            self.skip_newlines()?;
        }
        self.ast_builder.finish_node();
        Ok(())
    }

    fn parse_expr(&mut self, precedence: u8) -> LanguloResult<()> {
        let checkpoint = self.ast_builder.checkpoint();

        self.parse_prefix()?;

        loop {
            let next_precedence = match self.peek_meaningful_token()? {
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

    fn parse_prefix(&mut self) -> LanguloResult<()> {
        let tok = self.next_meaningful_token()?;
        match tok {
            Tok::Num(value) => self.add_leaf_node(AstNode::Num, value),
            Tok::Literal(value) => self.add_leaf_node(AstNode::Literal, value),
            Tok::QuestionMark => self.add_leaf_node(AstNode::NoOption, "?"),
            Tok::StringLitDouble(value) => self.parse_string_lit(value, "\"")?,
            Tok::StringLitSingle(value) => self.parse_string_lit(value, "'")?,
            Tok::TripleDoubleQuote(value) => self.parse_string_lit(value, "\"\"\"")?,
            Tok::TripleSingleQuote(value) => self.parse_string_lit(value, "'''")?,
            Tok::True(value) | Tok::False(value) => self.add_leaf_node(AstNode::Bool, value),
            Tok::At => {
                if !self.is_in_fn_body {
                    return Err(LanguloError::AtOutsideFunctionDeclaration {
                        _src: self.source.into(),
                        _span: self.span_highlighting_token(&tok),
                    });
                }
                self.add_leaf_node(AstNode::Literal, "@")
            }
            Tok::Not(_) => self.add_unary_node_prefix(AstNode::Not, tok.precedence())?,
            Tok::Dollar => self.add_unary_node_prefix(AstNode::Print, tok.precedence())?,
            Tok::Ask(_) => self.add_unary_node_prefix(AstNode::Ask, tok.precedence())?,
            Tok::Del(_) => self.add_unary_node_prefix(AstNode::Del, tok.precedence())?,
            Tok::LParen => {
                self.ast_builder.start_node(AstNode::Grouping.into());
                self.parse_expr(0)?;
                self.consume_required_tok(Tok::RParen)?;
                self.ast_builder.finish_node();
            }
            Tok::Pipe => self.parse_function_declaration()?,
            Tok::Minus => {
                // desugar - <expr> into -1 * <expr>
                self.ast_builder.start_node(AstNode::Multiply.into());
                self.add_leaf_node(AstNode::Num, "-1");
                self.parse_expr(Tok::Star.precedence())?;
                self.ast_builder.finish_node();
            }
            Tok::LBrace => {
                self.ast_builder.start_node(AstNode::Block.into());
                self.parse_braced_exprs()?;
                self.ast_builder.finish_node();
            }
            Tok::Return(value) => {
                self.ast_builder.start_node(AstNode::Return.into());
                self.ast_builder.token(AstNode::Return.into(), value);
                self.parse_expr(0)?;
                self.ast_builder.finish_node();
            }

            Tok::If(_) => {
                // if cond expr
                self.ast_builder.start_node(AstNode::If.into());
                self.parse_expr(0)?;
                self.consume_required_tok(Tok::Colon)?;
                self.parse_expr(Tok::Else("").precedence())?;
                self.ast_builder.finish_node();
            }

            Tok::LBracket => self.parse_map()?,

            Tok::Set(_) => {
                self.ast_builder.start_node(AstNode::SetLit.into());
                self.consume_required_tok(Tok::LBracket)?;
                self.parse_bracket_items()?;
                self.ast_builder.finish_node();
            }

            Tok::List(_) => {
                self.ast_builder.start_node(AstNode::ListLit.into());
                self.consume_required_tok(Tok::LBracket)?;
                self.parse_bracket_items()?;
                self.ast_builder.finish_node();
            }

            Tok::Key(value) | Tok::Value(value) | Tok::Index(value) => {
                if !self.is_in_iter_body {
                    return Err(LanguloError::IterVarOutsideIter {
                        _src: self.source.into(),
                        _span: self.span_highlighting_token(&tok),
                        _var: value.into(),
                    });
                }
                self.add_leaf_node(AstNode::IterVar, value);
            }

            _ => {
                return Err(LanguloError::UnexpectedToken {
                    _token: tok.info(),
                    _expected: "a prefix operand".into(),
                    _src: self.source.into(),
                    _span: self.span_highlighting_token(&tok),
                });
            }
        }
        Ok(())
    }

    fn parse_braced_exprs(&mut self) -> LanguloResult<()> {
        self.skip_newlines()?;
        if let Tok::RBrace = self.peek_meaningful_token_or_eof_err()? {
            return Err(LanguloError::EmptyBlock {
                _src: self.source.into(),
                _span: (self.current_offset, 1).into(),
            });
        }
        loop {
            if let Tok::RBrace = self.peek_meaningful_token_or_eof_err()? {
                break;
            }
            self.parse_expr(0)?;
            self.skip_newlines()?;
        }
        self.consume_required_tok(Tok::RBrace)?;
        Ok(())
    }

    fn add_leaf_node(&mut self, node: AstNode, value: &str) {
        self.ast_builder.start_node(node.into());
        self.ast_builder.token(node.into(), value);
        self.ast_builder.finish_node();
    }

    fn parse_infix(&mut self, checkpoint: Checkpoint, precedence: u8) -> LanguloResult<()> {
        let tok = self.next_meaningful_token()?;
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
            Tok::Else(_) => self.add_binary_node(AstNode::Else, checkpoint, precedence)?,
            Tok::DotDot(_) => self.add_binary_node(AstNode::Range, checkpoint, precedence)?,
            Tok::LBracket => {
                self.add_binary_node(AstNode::MapIndex, checkpoint, precedence)?;
                self.consume_required_tok(Tok::RBracket)?;
            }

            Tok::ExclamationMark => self.add_unary_node_infix(AstNode::SomeOption, checkpoint),

            Tok::Dollar => {
                // print operator can also act on infix operators themselves
                // e.g., 3 +$ 4 prints 7
                self.ast_builder
                    .start_node_at(checkpoint, AstNode::Print.into());
                // make sure that the rest gets parsed with the right precedence, as if printing wasn't being parsed
                let next_precedence = self.peek_meaningful_token_or_eof_err()?.precedence();
                self.parse_infix(checkpoint, next_precedence)?;
                self.ast_builder.finish_node();
            }
            Tok::LParen => {
                self.ast_builder
                    .start_node_at(checkpoint, AstNode::PrefixFnCall.into());
                self.parse_call_args()?;
                self.ast_builder.finish_node();
            }
            Tok::Dot => {
                self.ast_builder
                    .start_node_at(checkpoint, AstNode::PostfixFnCall.into());
                self.parse_expr(precedence)?;
                self.ast_builder.finish_node();
            }
            Tok::Iter(_) => {
                self.ast_builder
                    .start_node_at(checkpoint, AstNode::Iter.into());
                self.consume_required_tok(Tok::LBrace)?;

                let was_in_iter = self.is_in_iter_body;
                self.is_in_iter_body = true;

                self.parse_braced_exprs()?;

                self.is_in_iter_body = was_in_iter;
                self.ast_builder.finish_node();
            }
            _ => {
                return Err(LanguloError::UnexpectedToken {
                    _token: tok.info(),
                    _expected: "an infix operand".into(),
                    _src: self.source.into(),
                    _span: self.span_highlighting_token(&tok),
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

    fn add_unary_node_infix(&mut self, node: AstNode, checkpoint: Checkpoint) {
        self.ast_builder.start_node_at(checkpoint, node.into());
        self.ast_builder.finish_node();
    }

    /// moves the lexer forward to the next statement, if there's any, in the scope
    /// (e.g., the root scope or a block scope) that's being parsed.
    fn skip_newlines(&mut self) -> LanguloResult<()> {
        while let Some(Tok::Newline(_)) = self.peek_meaningful_token()? {
            self.next_meaningful_token()?;
        }
        Ok(())
    }

    /// moves the lexer forward to the next token with semantic meaning
    fn skip_trivia(&mut self) -> LanguloResult<()> {
        while let Some(Ok(tok)) = self.lexer.peek() {
            match tok {
                Tok::Whitespace(slice)
                | Tok::BlockComment(slice)
                | Tok::LineContinuation(slice)
                | Tok::LineComment(slice) => {
                    self.current_offset += slice.len();
                    if self.current_offset > self.source.len() {
                        return Err(LanguloError::InternalError {
                            _message: format!("offset went outside of source code because of tok {:?}", tok),
                        })
                    }
                    self.lexer.next();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn consume_required_tok(&mut self, required_tok: Tok) -> LanguloResult<()> {
        let found_tok = self.peek_meaningful_token_or_eof_err()?;

        if found_tok == required_tok {
            let _ = self.next_meaningful_token()?;
            Ok(())
        } else {
            Err(LanguloError::UnexpectedToken {
                _token: found_tok.info(),
                _expected: required_tok.info(),
                _src: self.source.into(),
                _span: self.span_highlighting_token(&found_tok),
            })
        }
    }

    fn parse_function_declaration(&mut self) -> LanguloResult<()> {
        self.ast_builder.start_node(AstNode::FunctionDecl.into());
        self.is_in_fn_body = true;
        self.parse_function_declaration_params()?;

        self.ast_builder.start_node(AstNode::FunctionBody.into());
        self.parse_expr(0)?;
        self.ast_builder.finish_node();

        self.ast_builder.finish_node();
        self.is_in_fn_body = false;
        Ok(())
    }

    fn parse_function_declaration_params(&mut self) -> LanguloResult<()> {
        // plus = |@, n| @ + n
        // parses "|@, n|"
        self.ast_builder.start_node(AstNode::FunctionParams.into());
        // early exit on empty decl
        if let Tok::Pipe = self.peek_meaningful_token_or_eof_err()? {
            self.next_meaningful_token()?;
            self.ast_builder.finish_node();
            return Ok(());
        }
        loop {
            match self.next_meaningful_token()? {
                Tok::At => self.add_leaf_node(AstNode::Literal, "@"),
                Tok::Literal(value) => self.add_leaf_node(AstNode::Literal, value),
                other => {
                    return Err(LanguloError::UnexpectedToken {
                        _token: other.info(),
                        _expected: "one of '|', '@' or a literal".into(),
                        _src: self.source.into(),
                        _span: self.span_highlighting_token(&other),
                    });
                }
            }
            if !self.should_keep_parsing_comma_list(Tok::Pipe)? {
                break;
            }
        }

        self.consume_required_tok(Tok::Pipe)?;
        self.ast_builder.finish_node();
        Ok(())
    }

    fn parse_call_args(&mut self) -> LanguloResult<()> {
        self.ast_builder.start_node(AstNode::CallArgs.into());

        // early exit on empty call
        if let Tok::RParen = self.peek_meaningful_token_or_eof_err()? {
            self.next_meaningful_token()?;
            self.ast_builder.finish_node();
            return Ok(());
        }
        loop {
            self.parse_expr(0)?;
            match self.peek_meaningful_token_or_eof_err()? {
                Tok::Comma => self.next_meaningful_token()?,
                Tok::RParen => break,
                other => {
                    return Err(LanguloError::UnexpectedToken {
                        _token: other.info(),
                        _expected: "one of ',' or ')'".into(),
                        _src: self.source.into(),
                        _span: (self.current_offset, 1).into(),
                    });
                }
            };
        }
        self.consume_required_tok(Tok::RParen)?;
        self.ast_builder.finish_node();
        Ok(())
    }

    /// Produces a [StringLit node](AstNode::StringLit).
    ///
    /// `"an {interpolated} string {literal}"` gets parsed into
    ///
    /// ```StringLit
    ///         StringPart          //an
    ///         InterpolationPart   // {interpolated}
    ///         StringPart          // string,
    ///         InterpolationPart   // {literal}
    /// ```
    fn parse_string_lit(&mut self, value: &str, quote: &str) -> LanguloResult<()> {
        // calculate string bounds excluding quotes
        let token_start = self.current_offset - value.len();
        let content_start = token_start + quote.len();
        let content_end = self.current_offset - quote.len();

        self.ast_builder.start_node(AstNode::StringLit.into());
        self.ast_builder.token(AstNode::StringLit.into(), value);

        let mut is_slice_interpolation = false;

        let mut slice_start = content_start;
        let mut i = content_start;

        while i < content_end {
            let increment = match (self.source.as_bytes()[i], is_slice_interpolation) {
                (b'\\', _) => 2, // skip both \ and the char it's escaping
                (b'{', false) => {
                    // entering interpolation - flush current string part if any
                    if i > slice_start {
                        self.add_string_part(slice_start, i, quote);
                    }
                    is_slice_interpolation = true;
                    slice_start = i + 1;
                    1
                }
                (b'}', true) => {
                    // exiting interpolation - flush interpolation part
                    is_slice_interpolation = false;
                    self.add_interpolation_part(slice_start, i)?;
                    slice_start = i + 1;
                    1
                }
                (b'{', true) => {
                    return Err(LanguloError::LBraceInsideStringInterpolation {
                        _src: self.source.into(),
                        _span: (slice_start, i - slice_start).into(),
                    });
                }
                _ => 1,
            };
            i = min(i + increment, content_end); // make sure we don't overflow
        }
        if is_slice_interpolation {
            return Err(LanguloError::UnterminatedInterpolation {
                _src: self.source.into(),
                _span: (slice_start, content_end - slice_start).into(),
            });
        }

        // add any potential leftover string part
        if i > slice_start {
            self.add_string_part(slice_start, i, quote);
        }
        self.ast_builder.finish_node();
        Ok(())
    }

    fn add_interpolation_part(&mut self, start: usize, end: usize) -> LanguloResult<()> {
        self.ast_builder
            .start_node(AstNode::InterpolationPart.into());

        // Create a new lexer for the expression slice
        let expr_source = &self.source[start..end];
        self.ast_builder
            .token(AstNode::InterpolationPart.into(), expr_source);
        let inner_lexer = Lexer::new(expr_source).peekable();

        // Swap lexers and offset, parse interpolated expr, and restore them
        let outer_lexer = std::mem::replace(&mut self.lexer, inner_lexer);
        let outer_is_in_fn_body = self.is_in_fn_body;
        let outer_offset = self.current_offset;
        self.current_offset = start; // Keep offset relative to original source for errors

        self.parse_expr(0)?;

        self.lexer = outer_lexer;
        self.current_offset = outer_offset;
        self.is_in_fn_body = outer_is_in_fn_body;

        self.ast_builder.finish_node();
        Ok(())
    }

    fn add_string_part(&mut self, start: usize, end: usize, quote: &str) {
        self.ast_builder.start_node(AstNode::StringPart.into());
        // Store with quote info so transpiler knows how to emit it
        let quoted = format!("{}{}{}", quote, &self.source[start..end], quote);
        self.ast_builder.token(AstNode::StringPart.into(), &quoted);
        self.ast_builder.finish_node();
    }

    fn parse_map(&mut self) -> LanguloResult<()> {
        self.ast_builder.start_node(AstNode::MapLit.into());
        if let Tok::RBracket = self.peek_meaningful_token_or_eof_err()? {
            self.next_meaningful_token()?;
            self.ast_builder.finish_node();
            return Ok(());
        }

        loop {
            self.ast_builder.start_node(AstNode::MapEntry.into());
            self.parse_expr(0)?;
            self.consume_required_tok(Tok::Colon)?;
            self.parse_expr(0)?;
            self.ast_builder.finish_node();

            if !self.should_keep_parsing_comma_list(Tok::RBracket)? {
                break;
            };
        }

        self.consume_required_tok(Tok::RBracket)?;
        self.ast_builder.finish_node();
        Ok(())
    }

    fn should_keep_parsing_comma_list(&mut self, end_token: Tok) -> LanguloResult<bool> {
        match self.peek_meaningful_token_or_eof_err()? {
            Tok::Comma => {
                self.next_meaningful_token()?;
                Ok(true)
            }
            other => {
                if other == end_token {
                    Ok(false)
                } else {
                    Err(LanguloError::UnexpectedToken {
                        _token: other.info(),
                        _expected: format!("one of ',' or '{}'", end_token.info()),
                        _src: self.source.into(),
                        _span: self.span_highlighting_token(&other),
                    })
                }
            }
        }
    }

    fn parse_bracket_items(&mut self) -> LanguloResult<()> {
        if let Tok::RBracket = self.peek_meaningful_token_or_eof_err()? {
            self.next_meaningful_token()?;
            return Ok(());
        }

        loop {
            self.parse_expr(0)?;
            if !self.should_keep_parsing_comma_list(Tok::RBracket)? {
                break;
            }
        }

        self.consume_required_tok(Tok::RBracket)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::AstNode::*;
    use crate::parser::{has_print_marker, parse, AstNode};

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
            "AST node mismatch for '{}'",
            exp.source
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
            nodes: &[
                Root, Subtract, Add, Num, Multiply, Num, Num, Divide, Num, Num,
            ],
            children: &[&[1, 2, 7], &[2, 3, 4], &[4, 5, 6], &[7, 8, 9]],
            ..Default::default()
        });
    }

    #[test]
    fn test_unary_negation() {
        expect_ast(AstExpectation {
            source: "-3",
            nodes: &[Root, Multiply, Num, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_negation_in_expression() {
        // -3 + 4 should be ((-1 * 3) + 4)
        expect_ast(AstExpectation {
            source: "-3 + 4",
            nodes: &[Root, Add, Multiply, Num, Num, Num],
            children: &[&[1, 2, 5], &[2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_negation_in_call() {
        expect_ast(AstExpectation {
            source: "foo(-3)",
            nodes: &[Root, PrefixFnCall, Literal, CallArgs, Multiply, Num, Num],
            children: &[&[1, 2, 3], &[3, 4], &[4, 5, 6]],
            ..Default::default()
        });
    }

    #[test]
    fn test_double_negation() {
        // --3 should be (-1) * ((-1) * 3)
        expect_ast(AstExpectation {
            source: "--3",
            nodes: &[Root, Multiply, Num, Multiply, Num, Num],
            children: &[&[1, 2, 3], &[3, 4, 5]],
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
            nodes: &[Root, Not, Xor, And, Bool, Bool, Bool],
            children: &[&[1, 2], &[2, 3, 6], &[3, 4, 5]],
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
    /////////////
    // fn decl //
    /////////////
    #[test]
    fn test_function_simple() {
        expect_ast(AstExpectation {
            source: "|n,m|n+m",
            nodes: &[
                Root,
                FunctionDecl,
                FunctionParams,
                Literal,
                Literal,
                FunctionBody,
                Add,
                Literal,
                Literal,
            ],
            children: &[&[1, 2, 5], &[2, 3, 4], &[5, 6], &[6, 7, 8]],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_with_at_param() {
        expect_ast(AstExpectation {
            source: "|@,other|@+other",
            nodes: &[
                Root,
                FunctionDecl,
                FunctionParams,
                Literal,
                Literal,
                FunctionBody,
                Add,
                Literal,
                Literal,
            ],
            children: &[&[1, 2, 5], &[2, 3, 4], &[5, 6], &[6, 7, 8]],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_no_params() {
        expect_ast(AstExpectation {
            source: "||42",
            nodes: &[Root, FunctionDecl, FunctionParams, FunctionBody, Num],
            children: &[&[1, 2, 3], &[3, 4]],
            ..Default::default()
        });
    }
    //////////////
    // fn calls //
    //////////////

    #[test]
    fn test_function_call_no_args() {
        expect_ast(AstExpectation {
            source: "foo()",
            nodes: &[Root, PrefixFnCall, Literal, CallArgs],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_call_one_arg() {
        expect_ast(AstExpectation {
            source: "foo(1)",
            nodes: &[Root, PrefixFnCall, Literal, CallArgs, Num],
            children: &[&[1, 2, 3], &[3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_call_two_args() {
        expect_ast(AstExpectation {
            source: "add(1, 2)",
            nodes: &[Root, PrefixFnCall, Literal, CallArgs, Num, Num],
            children: &[&[1, 2, 3], &[3, 4, 5]],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_call_complex_args() {
        expect_ast(AstExpectation {
            source: "foo(1 + 2, 3 * 4)",
            nodes: &[
                Root,
                PrefixFnCall,
                Literal,
                CallArgs,
                Add,
                Num,
                Num,
                Multiply,
                Num,
                Num,
            ],
            children: &[&[1, 2, 3], &[3, 4, 7], &[4, 5, 6], &[7, 8, 9]],
            ..Default::default()
        });
    }

    #[test]
    fn test_postfix_call_simple() {
        // 3 @ plus(2) -> PostfixFnCall(Num(3), PrefixFnCall(plus, CallArgs(2)))
        expect_ast(AstExpectation {
            source: "3.plus(2)",
            nodes: &[
                Root,
                PostfixFnCall,
                Num,
                PrefixFnCall,
                Literal,
                CallArgs,
                Num,
            ],
            children: &[&[1, 2, 3], &[3, 4, 5], &[5, 6]],
            ..Default::default()
        });
    }

    #[test]
    fn test_postfix_call_no_extra_args() {
        // 3 @ double() -> PostfixFnCall(Num(3), PrefixFnCall(double, CallArgs))
        expect_ast(AstExpectation {
            source: "3 .double()",
            nodes: &[Root, PostfixFnCall, Num, PrefixFnCall, Literal, CallArgs],
            children: &[&[1, 2, 3], &[3, 4, 5]],
            ..Default::default()
        });
    }

    #[test]
    fn test_postfix_call_chained() {
        // 3 @ plus(2) @ times(4)
        expect_ast(AstExpectation {
            source: "3 .plus(2) .times(4)",
            nodes: &[
                Root,
                PostfixFnCall,
                PostfixFnCall,
                Num,
                PrefixFnCall,
                Literal,
                CallArgs,
                Num,
                PrefixFnCall,
                Literal,
                CallArgs,
                Num,
            ],
            children: &[
                &[1, 2, 8],
                &[2, 3, 4],
                &[4, 5, 6],
                &[6, 7],
                &[8, 9, 10],
                &[10, 11],
            ],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_call_in_expression() {
        expect_ast(AstExpectation {
            source: "1 + foo(2) * 3",
            nodes: &[
                Root,
                Add,
                Num,
                Multiply,
                PrefixFnCall,
                Literal,
                CallArgs,
                Num,
                Num,
            ],
            children: &[&[1, 2, 3], &[3, 4, 8], &[4, 5, 6], &[6, 7]],
            ..Default::default()
        });
    }

    #[test]
    fn test_block_single_expr() {
        expect_ast(AstExpectation {
            source: "{ 42 }",
            nodes: &[Root, Block, Num],
            children: &[&[1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_block_multiple_statements() {
        expect_ast(AstExpectation {
            source: "{ 1\n2\n3 }",
            nodes: &[Root, Block, Num, Num, Num],
            children: &[&[1, 2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_block_with_assignments() {
        expect_ast(AstExpectation {
            source: "{ x = 1\ny = 2\nx + y }",
            nodes: &[
                Root, Block, Assign, Literal, Num, Assign, Literal, Num, Add, Literal, Literal,
            ],
            children: &[&[1, 2, 5, 8], &[2, 3, 4], &[5, 6, 7], &[8, 9, 10]],
            ..Default::default()
        });
    }

    #[test]
    fn test_block_with_return() {
        expect_ast(AstExpectation {
            source: "{ return 42 }",
            nodes: &[Root, Block, Return, Num],
            children: &[&[1, 2], &[2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_block_return_early() {
        expect_ast(AstExpectation {
            source: "{ return 1\n2 }",
            nodes: &[Root, Block, Return, Num, Num],
            children: &[&[1, 2, 4], &[2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_nested_blocks() {
        expect_ast(AstExpectation {
            source: "{ { 1 } }",
            nodes: &[Root, Block, Block, Num],
            children: &[&[1, 2], &[2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_block_in_expression() {
        expect_ast(AstExpectation {
            source: "1 + { 2 }",
            nodes: &[Root, Add, Num, Block, Num],
            children: &[&[1, 2, 3], &[3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_line_continuation() {
        // 1 + \n 2 should parse as single expression
        expect_ast(AstExpectation {
            source: "1 +\\\n2",
            nodes: &[Root, Add, Num, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_line_continuation_with_indent() {
        expect_ast(AstExpectation {
            source: "1\\\n    + 2",
            nodes: &[Root, Add, Num, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_multiple_statements_root() {
        expect_ast(AstExpectation {
            source: "1\n2\n3",
            nodes: &[Root, Num, Num, Num],
            children: &[&[0, 1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_empty_lines_ignored() {
        expect_ast(AstExpectation {
            source: "\n\n1\n\n2\n\n",
            nodes: &[Root, Num, Num],
            children: &[&[0, 1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_function_with_block_body() {
        expect_ast(AstExpectation {
            source: "f = |x| { y = x + 1\nreturn y }",
            nodes: &[
                Root,
                Assign,
                Literal,
                FunctionDecl,
                FunctionParams,
                Literal,
                FunctionBody,
                Block,
                Assign,
                Literal,
                Add,
                Literal,
                Num,
                Return,
                Literal,
            ],
            ..Default::default()
        });
    }

    #[test]
    fn test_empty_block_error() {
        let result = parse("{ }");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_block_with_newlines_error() {
        let result = parse("{\n\n}");
        assert!(result.is_err());
    }

    #[test]
    fn test_string_simple() {
        expect_ast(AstExpectation {
            source: r#""hello""#,
            nodes: &[Root, StringLit, StringPart],
            children: &[&[1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_string_single_quotes() {
        expect_ast(AstExpectation {
            source: r#"'hello'"#,
            nodes: &[Root, StringLit, StringPart],
            children: &[&[1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_only_iterpolation() {
        expect_ast(AstExpectation {
            source: r#""{x}""#,
            nodes: &[Root, StringLit, InterpolationPart, Literal],
            children: &[&[1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_string_with_interpolation() {
        expect_ast(AstExpectation {
            source: r#""hello {name}""#,
            nodes: &[Root, StringLit, StringPart, InterpolationPart, Literal],
            children: &[&[1, 2, 3], &[3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_string_multiple_interpolations() {
        expect_ast(AstExpectation {
            source: r#""a {x} b {y} c""#,
            nodes: &[
                Root,
                StringLit,
                StringPart,
                InterpolationPart,
                Literal,
                StringPart,
                InterpolationPart,
                Literal,
                StringPart,
            ],
            children: &[&[1, 2, 3, 5, 6, 8], &[3, 4], &[6, 7]],
            ..Default::default()
        });
    }

    #[test]
    fn test_string_interpolation_with_expr() {
        expect_ast(AstExpectation {
            source: r#""result: {1 + 2}""#,
            nodes: &[
                Root,
                StringLit,
                StringPart,
                InterpolationPart,
                Add,
                Num,
                Num,
            ],
            children: &[&[1, 2, 3], &[3, 4], &[4, 5, 6]],
            ..Default::default()
        });
    }

    #[test]
    fn test_string_nested_quotes() {
        // Double quoted string with single quoted string in interpolation
        expect_ast(AstExpectation {
            source: r#""hello {'world'}""#,
            nodes: &[
                Root,
                StringLit,
                StringPart,
                InterpolationPart,
                StringLit,
                StringPart,
            ],
            children: &[&[1, 2, 3], &[3, 4], &[4, 5]],
            ..Default::default()
        });
    }

    #[test]
    fn test_string_concatenation() {
        expect_ast(AstExpectation {
            source: r#""hello" + " world""#,
            nodes: &[Root, Add, StringLit, StringPart, StringLit, StringPart],
            children: &[&[1, 2, 4], &[2, 3], &[4, 5]],
            ..Default::default()
        });
    }

    #[test]
    fn test_multiline_string() {
        expect_ast(AstExpectation {
            source: r#""""hello
world""""#,
            nodes: &[Root, StringLit, StringPart],
            children: &[&[1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_some_literal() {
        expect_ast(AstExpectation {
            source: "42!",
            nodes: &[Root, SomeOption, Num],
            children: &[&[1, 2]],
            ..Default::default()
        });
    }

    #[test]
    fn test_none_literal() {
        expect_ast(AstExpectation {
            source: "?",
            nodes: &[Root, NoOption],
            ..Default::default()
        });
    }

    #[test]
    fn test_else_with_some() {
        expect_ast(AstExpectation {
            source: "2! else 3",
            nodes: &[Root, Else, SomeOption, Num, Num],
            children: &[&[1, 2, 4], &[2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_else_with_none() {
        expect_ast(AstExpectation {
            source: "? else 4",
            nodes: &[Root, Else, NoOption, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_if_false() {
        expect_ast(AstExpectation {
            source: "if false: 1",
            nodes: &[Root, If, Bool, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_if_else_chain() {
        // if true 2 else 3 -> (if true 2) else 3
        expect_ast(AstExpectation {
            source: "if true: 2 else 3",
            nodes: &[Root, Else, If, Bool, Num, Num],
            children: &[&[1, 2, 5], &[2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_if_with_grouped_condition() {
        expect_ast(AstExpectation {
            source: "if (x > 0): 42",
            nodes: &[Root, If, Grouping, Gt, Literal, Num, Num],
            children: &[&[1, 2, 6], &[2, 3], &[3, 4, 5]],
            ..Default::default()
        });
    }

    #[test]
    fn test_if_else_chain_complex_body() {
        expect_ast(AstExpectation {
            source: "if true: (2+3) else 3",
            nodes: &[Root, Else, If, Bool, Grouping, Add, Num, Num, Num],
            children: &[&[1, 2, 8], &[2, 3, 4], &[4, 5], &[5, 6, 7]],
            ..Default::default()
        });
    }

    #[test]
    fn test_empty_map() {
        expect_ast(AstExpectation {
            source: "[]",
            nodes: &[Root, MapLit],
            ..Default::default()
        });
    }

    #[test]
    fn test_map_literal() {
        expect_ast(AstExpectation {
            source: "[1: 2, 3: 4]",
            nodes: &[Root, MapLit, MapEntry, Num, Num, MapEntry, Num, Num],
            children: &[&[1, 2, 5], &[2, 3, 4], &[5, 6, 7]],
            ..Default::default()
        });
    }

    #[test]
    fn test_set_literal() {
        expect_ast(AstExpectation {
            source: "set[1, 2, 3]",
            nodes: &[Root, SetLit, Num, Num, Num],
            children: &[&[1, 2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_list_literal() {
        expect_ast(AstExpectation {
            source: "list[1, 2, 3]",
            nodes: &[Root, ListLit, Num, Num, Num],
            children: &[&[1, 2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_range() {
        expect_ast(AstExpectation {
            source: "1..5",
            nodes: &[Root, Range, Num, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_map_index() {
        expect_ast(AstExpectation {
            source: "map[1]",
            nodes: &[Root, MapIndex, Literal, Num],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_map_index_assignment() {
        expect_ast(AstExpectation {
            source: "map[1] = 2",
            nodes: &[Root, Assign, MapIndex, Literal, Num, Num],
            children: &[&[1, 2, 5], &[2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_del() {
        expect_ast(AstExpectation {
            source: "del map[1]",
            nodes: &[Root, Del, MapIndex, Literal, Num],
            children: &[&[1, 2], &[2, 3, 4]],
            ..Default::default()
        });
    }

    #[test]
    fn test_iter_simple() {
        expect_ast(AstExpectation {
            source: "map iter { key }",
            nodes: &[Root, Iter, Literal, IterVar],
            children: &[&[1, 2, 3]],
            ..Default::default()
        });
    }

    #[test]
    fn test_iter_with_all_vars() {
        expect_ast(AstExpectation {
            source: "map iter { key + value + index }",
            nodes: &[Root, Iter, Literal, Add, Add, IterVar, IterVar, IterVar],
            children: &[&[1, 2, 3], &[3, 4, 7], &[4, 5, 6]],
            ..Default::default()
        });
    }

    #[test]
    fn test_del_with_else() {
        // del m[1] else 0 -> (del m[1]) else 0
        expect_ast(AstExpectation {
            source: "del m[1] else 0",
            nodes: &[Root, Else, Del, MapIndex, Literal, Num, Num],
            children: &[&[1, 2, 6], &[2, 3], &[3, 4, 5]],
            ..Default::default()
        });
    }

    #[test]
    fn test_str_indexing() {
        expect_ast(AstExpectation {
            source: "\"hi\"[1]",
            nodes: &[Root, MapIndex, StringLit, StringPart, Num],
            ..Default::default()
        })
    }
}
