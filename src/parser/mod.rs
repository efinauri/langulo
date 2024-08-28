mod precedence;
pub mod ast;

use crate::errors::err::LanguloErr;
use crate::lexer::tok::Tok;
use crate::lexer::Lexer;
use crate::parser::ast::lang::LanguloSyntaxNode;
use crate::parser::ast::node::AstNode;
use rowan::Checkpoint;

pub type ASTBuilder = rowan::GreenNodeBuilder<'static>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    builder: ASTBuilder,
}

// macro to avoid double mut borrow
macro_rules! next {
    ($self:expr) => {{
        $self.skip_trivia()?;
        let result = $self.lexer.next()?.ok_or_else(|| LanguloErr::semantic("Unexpected EOF"))?;
        $self.skip_trivia()?;
        result
    }};
}

#[derive(Debug)]
enum SemicolonPolicy {
    RequiredPresent,
    RequiredAbsent,
    Optional,
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

    fn new_leaf_node(&mut self, expr: AstNode, content: &str) -> Result<(), LanguloErr> {
        self.builder.start_node(expr.into());
        self.builder.token(expr.into(), content);
        self.builder.finish_node();
        Ok(())
    }

    fn new_binary_node(&mut self, kind: AstNode, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        self.builder.start_node_at(checkpoint, kind.into());
        self.parse_expr(precedence, SemicolonPolicy::RequiredAbsent)?;
        self.builder.finish_node();
        Ok(())
    }

    fn new_prefix_unary_node(&mut self, kind: AstNode, tok: &Tok) -> Result<(), LanguloErr> {
        self.builder.start_node(kind.into());
        self.parse_expr(tok.precedence(), SemicolonPolicy::RequiredAbsent)?;
        self.builder.finish_node();
        Ok(())
    }

    fn new_postfix_unary_node(&mut self, kind: AstNode, checkpoint: Checkpoint) -> Result<(), LanguloErr> {
        self.builder.start_node_at(checkpoint, kind.into());
        self.builder.finish_node();
        Ok(())
    }

    pub fn parse(&mut self) -> Result<(), LanguloErr> {
        self.builder.start_node(AstNode::Root.into());
        while self.lexer.peek()?.is_some() {
            self.parse_expr(0, SemicolonPolicy::RequiredPresent)?;
        }
        self.builder.finish_node();
        Ok(())
    }

    fn parse_expr(&mut self, precedence: u8, check_semicolon: SemicolonPolicy) -> Result<(), LanguloErr> {
        let checkpoint = self.builder.checkpoint();

        self.parse_prefix()?;

        loop {
            let tok_precedence = match self.lexer.peek()? {
                Some((tok, _)) => tok.precedence(),
                None => break,
            };
            if tok_precedence <= precedence { break; }

            self.parse_postfix(checkpoint, tok_precedence)?;
        }
        self.handle_semicolon(check_semicolon)?;
        Ok(())
    }

    fn parse_prefix(&mut self) -> Result<(), LanguloErr> {
        let (tok, content) = next!(self);

        match tok {
            Tok::Int => self.new_leaf_node(AstNode::Int, content)?,
            Tok::Float => self.new_leaf_node(AstNode::Float, content)?,
            Tok::Bool => self.new_leaf_node(AstNode::Bool, content)?,
            Tok::Char => self.new_leaf_node(AstNode::Char, content)?,
            Tok::Str => self.new_leaf_node(AstNode::Str, content)?,
            Tok::Identifier => self.new_leaf_node(AstNode::Identifier, content)?,
            Tok::Not => self.new_prefix_unary_node(AstNode::LogicalNot, &tok)?,
            Tok::Dollar => self.new_prefix_unary_node(AstNode::Print, &tok)?,
            Tok::Pipe => self.parse_scope(AstNode::Lambda, Tok::Pipe)?,
            Tok::LParen => {
                self.builder.start_node(AstNode::Grouping.into());
                self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
                self.assert_tok(Tok::RParen)?;
                self.builder.finish_node();
            }
            Tok::LBrace => self.parse_scope(AstNode::Scope, Tok::RBrace)?,
            Tok::If => {
                self.builder.start_node(AstNode::If.into());
                self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?; // condition
                self.parse_expr(tok.precedence(), SemicolonPolicy::Optional)?; // body
                self.builder.finish_node();
            }
            Tok::Var => {
                self.builder.start_node(AstNode::VarDecl.into());
                let var_name = self.assert_tok(Tok::Identifier)?;
                self.builder.token(AstNode::VarDecl.into(), var_name);
                // optional type hint
                if matches!(self.lexer.peek()?, Some((Tok::Colon, _))) {
                    next!(self);
                    self.parse_type()?;
                }
                self.assert_tok(Tok::Assign)?;
                self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
                self.builder.finish_node();
            }
            Tok::LBracket => { // table value
                self.builder.start_node(AstNode::Table.into());
                let mut seen_default_key = false;

                while !matches!(self.lexer.peek()?, Some((Tok::RBracket, _))) {
                    self.builder.start_node(AstNode::TablePair.into());
                    // parse key paying attention to default key
                    if matches!(self.lexer.peek()?, Some((Tok::Underscore, _))) {
                        if seen_default_key {
                            return Err(LanguloErr::semantic("Default key already defined"));
                        }
                        next!(self);
                        seen_default_key = true;
                        self.new_leaf_node(AstNode::DefaultKey, "_")?;
                    } else {
                        self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
                    }

                    self.assert_tok(Tok::Colon)?;
                    self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
                    self.builder.finish_node();

                    if matches!(self.lexer.peek()?, Some((Tok::Comma, _))) {
                        next!(self);
                    } else { break; }
                }
                self.assert_tok(Tok::RBracket)?;
                self.builder.finish_node();
            }
            Tok::Fn => { // fn(@int, other int) int { it + other };
                self.builder.start_node(AstNode::FunctionDecl.into());
                self.assert_tok(Tok::LParen)?;
                // optional @param, force it to be the first one
                if matches!(self.lexer.peek()?, Some((Tok::At, _))) {
                    self.builder.start_node(AstNode::PrincipalParam.into());
                    next!(self);
                    self.parse_type()?;
                    self.builder.finish_node();
                    if matches!(self.lexer.peek()?, Some((Tok::Comma, _))) { next!(self); }
                }

                // contour params
                while !matches!(self.lexer.peek()?, Some((Tok::RParen, _))) {
                    let param_name = self.assert_tok(Tok::Identifier)?;
                    self.builder.start_node(AstNode::ContourParam.into());
                    self.builder.token(AstNode::ContourParam.into(), param_name);
                    self.parse_type()?;
                    self.builder.finish_node();
                    if matches!(self.lexer.peek()?, Some((Tok::Comma, _))) { next!(self); } else { break; }
                }
                self.assert_tok(Tok::RParen)?;
                // return type
                self.parse_type()?;
                // body
                self.assert_tok(Tok::LBrace)?;
                self.parse_scope(AstNode::Scope, Tok::RBrace)?;
                self.builder.finish_node();
            }
            Tok::At => { // @add(1, 2);
                self.builder.start_node(AstNode::FunctionAppl.into());
                self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
                self.assert_tok(Tok::LParen)?;

                self.builder.start_node(AstNode::ContourArgs.into());
                self.parse_comma_separated_exprs()?;
                self.builder.finish_node();
                self.assert_tok(Tok::RParen)?;

                self.builder.finish_node();
            }

            _ => return Err(LanguloErr::semantic(
                &*format!("Expected a literal or prefix operator, but found {}", content)
            ))
        }
        Ok(())
    }

    fn parse_scope(&mut self, scope_kind: AstNode, end_tok: Tok) -> Result<(), LanguloErr> {
        self.builder.start_node(scope_kind.into());
        while !matches!(self.lexer.peek()?, Some((bind, _)) if bind == &end_tok) {
            // since we don't know if this will be the last expr until we evaluate it,
            // disable semicolon evaluation in the recursive call, and do it manually after
            self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
            let on_last_scope_expr = matches!(self.lexer.peek()?, Some((bind, _)) if bind == &end_tok);
            self.handle_semicolon(if !on_last_scope_expr { SemicolonPolicy::RequiredPresent } else { SemicolonPolicy::Optional })?;
        }
        self.assert_tok(end_tok)?;
        self.builder.finish_node();
        Ok(())
    }

    fn parse_postfix(&mut self, checkpoint: Checkpoint, precedence: u8) -> Result<(), LanguloErr> {
        let (tok, content) = next!(self);

        match tok {
            Tok::Plus => self.new_binary_node(AstNode::Add, checkpoint, precedence)?,
            Tok::Minus => self.new_binary_node(AstNode::Subtract, checkpoint, precedence)?,
            Tok::Star => self.new_binary_node(AstNode::Multiply, checkpoint, precedence)?,
            Tok::Slash => self.new_binary_node(AstNode::Divide, checkpoint, precedence)?,
            Tok::Modulo => self.new_binary_node(AstNode::Modulo, checkpoint, precedence)?,
            Tok::And => self.new_binary_node(AstNode::LogicalAnd, checkpoint, precedence)?,
            Tok::Or => self.new_binary_node(AstNode::LogicalOr, checkpoint, precedence)?,
            Tok::Else => self.new_binary_node(AstNode::Else, checkpoint, precedence)?,
            Tok::Question => self.new_postfix_unary_node(AstNode::Option, checkpoint)?,
            Tok::At => { // 3@plus(2);
                self.builder.start_node_at(checkpoint, AstNode::FunctionAppl.into());

                // fn body
                self.parse_expr(tok.precedence(), SemicolonPolicy::RequiredAbsent)?;

                if matches!(self.lexer.peek()?, Some((Tok::LParen, _))) {
                    next!(self);
                    // contour args
                    self.builder.start_node(AstNode::ContourArgs.into());
                    self.parse_comma_separated_exprs()?;
                    self.assert_tok(Tok::RParen)?;
                    self.builder.finish_node();
                }

                self.builder.finish_node();
            }
            _ => return Err(LanguloErr::semantic(
                &*format!("Expected an infix or postfix operator, but found {}", content)
            ))
        }
        Ok(())
    }

    fn parse_comma_separated_exprs(&mut self) -> Result<(), LanguloErr> {
        while !matches!(self.lexer.peek()?, Some((Tok::RParen, _))) {
            self.parse_expr(0, SemicolonPolicy::RequiredAbsent)?;
            if matches!(self.lexer.peek()?, Some((Tok::Comma, _))) {
                next!(self);
            } else { break; }
        }
        Ok(())
    }

    fn parse_type(&mut self) -> Result<(), LanguloErr> {
        let checkpoint = self.builder.checkpoint();

        let (tok, content) = next!(self);
        match tok {
            Tok::TypeChar => self.new_leaf_node(AstNode::TypeChar, content)?,
            Tok::TypeInt => self.new_leaf_node(AstNode::TypeInt, content)?,
            Tok::TypeFloat => self.new_leaf_node(AstNode::TypeFloat, content)?,
            Tok::TypeBool => self.new_leaf_node(AstNode::TypeBool, content)?,
            Tok::TypeStr => self.new_leaf_node(AstNode::TypeStr, content)?,
            Tok::Fn => { // fn(@int, str, char, ->bool)
                self.builder.start_node(AstNode::TypeFn.into());
                self.assert_tok(Tok::LParen)?;
                // don't include @type in contour types
                if matches!(self.lexer.peek()?, Some((Tok::At, _))) {
                    next!(self);
                    self.parse_type()?;
                    // this can be asserted because a return type must be annotated
                    self.assert_tok(Tok::Comma)?;
                }
                self.builder.start_node(AstNode::ContourTypes.into());
                while !matches!(self.lexer.peek()?, Some((Tok::Minus, _))) {
                    self.parse_type()?;
                    self.assert_tok(Tok::Comma)?;
                    if matches!(self.lexer.peek()?, Some((Tok::Minus, _))) { break; }
                }
                self.builder.finish_node();
                // return type
                self.assert_tok(Tok::Minus)?;
                self.assert_tok(Tok::GreaterThan)?;
                self.parse_type()?;

                self.assert_tok(Tok::RParen)?;
                self.builder.finish_node();
            }
            Tok::LBracket => {
                self.builder.start_node(AstNode::TypeTable.into());
                // [int:char]
                self.parse_type()?;
                self.assert_tok(Tok::Colon)?;
                self.parse_type()?;
                self.assert_tok(Tok::RBracket)?;
                self.builder.finish_node();
            }
            _ => return Err(LanguloErr::semantic(&*format!("Expected a type annotation, but found {:?}", tok))),
        }

        // ? is the only postfix type annotation so explicit precedence handling is not needed
        while matches!(self.lexer.peek()?, Some((Tok::Question, _))) {
            next!(self);
            self.builder.start_node_at(checkpoint, AstNode::TypeOption.into());
            self.builder.finish_node();
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

    fn handle_semicolon(&mut self, policy: SemicolonPolicy) -> Result<(), LanguloErr> {
        // semicolons at eof are optional
        let some_tok = match self.lexer.peek()? {
            Some(tok) => tok,
            None => { return Ok(()) }
        };
        let next_is_semicolon = matches! { some_tok, (Tok::Semicolon, _) };
        match (policy, next_is_semicolon) {
            // this first condition maps to ok because the matched semicolon could be required by
            // the upper part of the call stack
            (SemicolonPolicy::RequiredAbsent, true)
            | (SemicolonPolicy::RequiredAbsent, false)
            | (SemicolonPolicy::Optional, false) => Ok(()),
            (SemicolonPolicy::RequiredPresent, false) => Err(LanguloErr::semantic("Expected end of expression")),
            (SemicolonPolicy::RequiredPresent, true)
            | (SemicolonPolicy::Optional, true) => {
                next!(self);
                Ok(())
            }
        }
    }

    fn assert_tok(&mut self, tok: Tok) -> Result<&'a str, LanguloErr> {
        println!("checking if {:?} matches {:?}", tok, self.lexer.peek()?);
        let matches = matches!(self.lexer.peek()?, Some((bind, _)) if bind == &tok);
        if matches {
            let (_, content) = next!(self);
            return Ok(content);
        }
        Err(LanguloErr::semantic(&*format!("Expected {:?}", tok)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::lang::LanguloSyntaxNode;

    pub fn to_simplified_string(node: &LanguloSyntaxNode) -> String {
        let children: Vec<String> = node.children().map(|c| to_simplified_string(&c)).collect();
        if node.kind() == AstNode::Root { return children.join("\n"); }

        let tok_str = node.text().to_string();
        let tok_str = tok_str.trim().split_whitespace().next().unwrap_or("");
        format!("<{:?}:{}>", node.kind(), tok_str);
        let node_fmt = format!("<{:?}:{}>", node.kind(), tok_str);

        if children.is_empty() {
            node_fmt
        } else if children.len() == 1 {
            format!("({} {})", node_fmt, children[0])
        } else if children.len() == 2 {
            format!("({} {} {})", children[0], node_fmt, children[1])
        } else {
            format!("({} [{}])", node_fmt, children.join(", "))
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
            "1 + 2 * (5 - 3);",
            "(<Int:1> <Add:1> (<Int:2> <Multiply:2> (<Grouping:5> (<Int:5> <Subtract:5> <Int:3>))))")
    }

    #[test]
    fn logical() {
        expect_parser("true and not false;", "(<Bool:true> <LogicalAnd:true> (<LogicalNot:false> <Bool:false>))")
    }

    #[test]
    fn multiexpr_program() {
        expect_parser(
            "1+2*3; true and not false;",
            "(<Int:1> <Add:123> (<Int:2> <Multiply:23> <Int:3>))\n(<Bool:true> <LogicalAnd:true> (<LogicalNot:false> <Bool:false>))",
        )
    }

    #[test]
    fn scope() {
        expect_parser("{1; 2; 3;};", "(<Scope:1> [<Int:1>, <Int:2>, <Int:3>])");
        // semicolon on the last expr is optional
        expect_parser("{1; 2; 3};", "(<Scope:1> [<Int:1>, <Int:2>, <Int:3>])")
    }

    #[test]
    fn if_else() {
        expect_parser(
            "if true {1} else {2};",
            "((<Bool:true> <If:true1> (<Scope:1> <Int:1>)) <Else:true1> (<Scope:2> <Int:2>))",
        )
    }

    #[test]
    fn variable_decl() {
        expect_parser(
            "var x = 5;",
            "(<VarDecl:x> <Int:5>)",
        );
        // with type hint
        expect_parser(
            "var x: int = 5;",
            "(<TypeInt:int> <VarDecl:x> <Int:5>)",
        )
    }

    #[test]
    fn table_decl_and_usage() {
        // base situation
        expect_parser(
            "var tbl = [1: 'a', 2: 'b', 3: 'c'];",
            "(<VarDecl:tbl> (<Table:1> [\
            (<Int:1> <TablePair:1> <Char:'a'>), \
            (<Int:2> <TablePair:2> <Char:'b'>), \
            (<Int:3> <TablePair:3> <Char:'c'>)\
            ]))",
        );
        // with type hint
        expect_parser(
            "var tbl: [int:char] = [];",
            "((<TypeInt:int> <TypeTable:intchar> <TypeChar:char>) <VarDecl:tbl> <Table:>)",
        );
        // with default arm
        expect_parser(
            "[1: 2, 3: 4, _: 1000]",
            "(<Table:1> [\
            (<Int:1> <TablePair:1> <Int:2>), \
            (<Int:3> <TablePair:3> <Int:4>), \
            (<DefaultKey:_> <TablePair:_> <Int:1000>)\
            ])",
        )
    }

    #[test]
    fn plain_identifier() {
        expect_parser("a", "<Identifier:a>")
    }

    #[test]
    fn functions_and_lambdas() {
        // standard declaration with principal argument
        expect_parser(
            "fn(@int, other int) int { it + other };",
            "(<FunctionDecl:int> [\
            (<PrincipalParam:int> <TypeInt:int>), \
            (<ContourParam:otherint> <TypeInt:int>), \
            <TypeInt:int>, \
            (<Scope:it> (<Identifier:it> <Add:it> <Identifier:other>))\
            ])",
        );
        // fn application with @arg
        expect_parser(
            "3 @plus(2, 1, 0);",
            "(<FunctionAppl:3plus2> [<Int:3>, <Identifier:plus>, (<ContourArgs:2> [\
            <Int:2>, <Int:1>, <Int:0>]\
            )])",
        );
        // () is optional if there's an @arg and no contour args
        expect_parser("3@repeat", "(<Int:3> <FunctionAppl:3repeat> <Identifier:repeat>)");

        // fn application without @arg
        expect_parser(
            "@noop(3);",
            "(<Identifier:noop> <FunctionAppl:noop3> (<ContourArgs:3> <Int:3>))",
        );
        // lambda declaration
        expect_parser(
            "|it|",
            "(<Lambda:it> <Identifier:it>)",
        );
        // type hint declaration
        expect_parser(
            "var add: fn(@int, int, ->int) = etc;",
            "((<TypeFn:int> [<TypeInt:int>, (<ContourTypes:int> <TypeInt:int>), <TypeInt:int>]) <VarDecl:add> <Identifier:etc>)",
        )
    }
    #[test]
    fn options() {
        expect_parser("3???;", "(<Option:3> (<Option:3> (<Option:3> <Int:3>)))");
        // decl with type hint
        expect_parser(
            "var x: int? = 3?;",
            "((<TypeOption:int> <TypeInt:int>) <VarDecl:x> (<Option:3> <Int:3>))",
        )
    }
}