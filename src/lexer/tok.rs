use logos::Logos;

#[derive(Debug, PartialEq, Logos)]
pub enum Tok {
    // this enum doesn't store the token payload.
    // instead, we carry the token's value as a slice on the input program's string.
    // this is because when the AST is built at a later point,
    // it's simpler if the node is just a (expr kind, expr string content) pair

    // parens
    #[regex("\\(")]
    LParen,
    #[regex("\\)")]
    RParen,
    #[regex("\\[")]
    LBracket,
    #[regex("\\]")]
    RBracket,
    #[regex(":")]
    Colon,
    #[regex("\\{")]
    LBrace,
    #[regex("}")]
    RBrace,

    // punctuation
    #[regex(",")]
    Comma,
    #[regex(";")]
    Semicolon,
    #[regex("\\.")]
    Dot,
    #[regex("\\|")]
    Pipe,

    // var and assign
    #[regex("var")]
    Var,
    #[regex("=")]
    Assign,
    #[regex("\\+=")]
    PlusAssign,
    #[regex("-=")]
    MinusAssign,
    #[regex("\\*=")]
    StarAssign,
    #[regex("/=")]
    SlashAssign,
    #[regex("\\^=")]
    CaretAssign,
    #[regex("%=")]
    ModuloAssign,

    // primitive types
    #[regex("int")]
    TypeInt,
    #[regex("float")]
    TypeFloat,
    #[regex("str")]
    TypeStr,
    #[regex("bool")]
    TypeBool,
    #[regex("char")]
    TypeChar,

    // primitive values
    #[regex("true|false")]
    Bool,
    #[regex(r"-?\d+")]
    Int,
    #[regex(r"-?\d+\.\d+")]
    Float,
    #[regex(r#""(?:[^"]|\\")*""#)]
    Str,
    #[regex(r"'.'")]
    Char,

    // arithmetic
    #[regex("\\+")]
    Plus,
    #[regex("-")]
    Minus,
    #[regex("\\*")]
    Star,
    #[regex("/")]
    Slash,
    #[regex("\\^")]
    Caret,
    #[regex("%")]
    Modulo,

    // logic
    #[regex("not")]
    Not,
    #[regex("and")]
    And,
    #[regex("or")]
    Or,
    #[regex("xor")]
    Xor,
    #[regex(">")]
    GreaterThan,
    #[regex("<")]
    LessThan,
    #[regex("==")]
    Equals,
    #[regex("!=")]
    NotEquals,
    #[regex(">=")]
    GreaterThanEq,
    #[regex("<=")]
    LessThanEq,

    // optionals, tables
    #[regex("if")]
    If,
    #[regex("else")]
    Else,
    #[regex("\\?")]
    Question,
    #[regex("no")]
    No,
    #[regex("!")]
    Bang,
    #[regex("_")]
    Underscore,
    #[regex("iter")]
    Iter,
    #[regex("list")]
    List,
    #[regex("set")]
    Set,
    #[regex("\\.\\.")]
    Range,

    // functions and structs
    #[regex("fn")]
    Fn,
    #[regex("@")]
    At,
    #[regex("return")]
    Return,
    #[regex("struct")]
    Struct,
    #[regex("new")]
    New,

    // others
    #[regex("\\$")]
    Dollar, // print
    #[regex("as")]
    As, // cast
    #[regex("\\s+")]
    Whitespace,
    #[regex("//.*")]
    Comment,
    #[regex("[A-Za-z][a-zA-Z_]*")]
    Identifier,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_lex(input: &str, toks: &[Tok]) {
        let mut lexer = Tok::lexer(input);
        for (i, tok) in toks.iter().enumerate() {
            println!("iteration: {i}, expecting {tok:?}");
            assert_eq!(
                &lexer
                    .next()
                    .expect(format!("expected {tok:?}, but lexer has nothing to parse").as_str())
                    .expect(format!("lexing error when expecting tok {tok:?}").as_str()),
                tok
            );
        }
        assert!(&lexer.next().is_none())
    }

    #[test]
    fn test_whitespace() {
        expect_lex(" \t\n  ", &[Tok::Whitespace]);
    }

    #[test]
    fn parens() {
        expect_lex(
            "()[]{}",
            &[
                Tok::LParen,
                Tok::RParen,
                Tok::LBracket,
                Tok::RBracket,
                Tok::LBrace,
                Tok::RBrace,
            ],
        );
    }

    #[test]
    fn punctuation_and_others() {
        expect_lex(
            ",;.|:$ as hello",
            &[
                Tok::Comma,
                Tok::Semicolon,
                Tok::Dot,
                Tok::Pipe,
                Tok::Colon,
                Tok::Dollar,
                Tok::Whitespace,
                Tok::As,
                Tok::Whitespace,
                Tok::Identifier,
            ],
        );
    }

    #[test]
    fn var_and_assign() {
        expect_lex(
            "var = += -= *= /= ^= %= ",
            &[
                Tok::Var,
                Tok::Whitespace,
                Tok::Assign,
                Tok::Whitespace,
                Tok::PlusAssign,
                Tok::Whitespace,
                Tok::MinusAssign,
                Tok::Whitespace,
                Tok::StarAssign,
                Tok::Whitespace,
                Tok::SlashAssign,
                Tok::Whitespace,
                Tok::CaretAssign,
                Tok::Whitespace,
                Tok::ModuloAssign,
                Tok::Whitespace,
            ],
        )
    }

    #[test]
    fn primitive_types() {
        expect_lex(
            "int float str bool char ",
            &[
                Tok::TypeInt,
                Tok::Whitespace,
                Tok::TypeFloat,
                Tok::Whitespace,
                Tok::TypeStr,
                Tok::Whitespace,
                Tok::TypeBool,
                Tok::Whitespace,
                Tok::TypeChar,
                Tok::Whitespace,
            ],
        )
    }

    #[test]
    fn values() {
        expect_lex(
            "123 123.456 \"hello\" 'c' true false -2 ",
            &[
                Tok::Int,
                Tok::Whitespace,
                Tok::Float,
                Tok::Whitespace,
                Tok::Str,
                Tok::Whitespace,
                Tok::Char,
                Tok::Whitespace,
                Tok::Bool,
                Tok::Whitespace,
                Tok::Bool,
                Tok::Whitespace,
                Tok::Int,
                Tok::Whitespace,
            ],
        );
    }

    #[test]
    fn arithmetic_and_logic() {
        expect_lex(
            "+ - * ^ % not and or xor < > >= <= == != ",
            &[
                Tok::Plus,
                Tok::Whitespace,
                Tok::Minus,
                Tok::Whitespace,
                Tok::Star,
                Tok::Whitespace,
                Tok::Caret,
                Tok::Whitespace,
                Tok::Modulo,
                Tok::Whitespace,
                Tok::Not,
                Tok::Whitespace,
                Tok::And,
                Tok::Whitespace,
                Tok::Or,
                Tok::Whitespace,
                Tok::Xor,
                Tok::Whitespace,
                Tok::LessThan,
                Tok::Whitespace,
                Tok::GreaterThan,
                Tok::Whitespace,
                Tok::GreaterThanEq,
                Tok::Whitespace,
                Tok::LessThanEq,
                Tok::Whitespace,
                Tok::Equals,
                Tok::Whitespace,
                Tok::NotEquals,
                Tok::Whitespace,
            ],
        )
    }

    #[test]
    fn options_and_tables() {
        expect_lex(
            "if else ? no ! _ iter list set .. ",
            &[
                Tok::If,
                Tok::Whitespace,
                Tok::Else,
                Tok::Whitespace,
                Tok::Question,
                Tok::Whitespace,
                Tok::No,
                Tok::Whitespace,
                Tok::Bang,
                Tok::Whitespace,
                Tok::Underscore,
                Tok::Whitespace,
                Tok::Iter,
                Tok::Whitespace,
                Tok::List,
                Tok::Whitespace,
                Tok::Set,
                Tok::Whitespace,
                Tok::Range,
                Tok::Whitespace,
            ],
        )
    }

    #[test]
    fn fn_and_structs() {
        expect_lex(
            "fn @ return struct new ",
            &[
                Tok::Fn,
                Tok::Whitespace,
                Tok::At,
                Tok::Whitespace,
                Tok::Return,
                Tok::Whitespace,
                Tok::Struct,
                Tok::Whitespace,
                Tok::New,
                Tok::Whitespace,
            ],
        )
    }

    #[test]
    fn errors() {
        for input in &["\"unterminated string", "'multichar char'"] {
            println!("asserting {}", input);
            assert!(Tok::lexer(input).next().is_some_and(|res| res.is_err()));
        }
    }
}
