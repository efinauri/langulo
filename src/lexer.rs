use logos::{Lexer, Logos};

// to make it easier to access token len, all tokens that are bigger than 1 char hold their slice, even if static
#[derive(Logos, Debug, PartialEq, Copy, Clone)]
pub enum Tok<'a> {
    #[regex(r"[a-zA-Z][a-zA-Z_]*")]
    Literal(&'a str),
    #[regex(r"[0-9][0-9_]*(\.[0-9_]+)?")]
    Num(&'a str),
    #[regex(r#""([^"\\]|\\.)*""#)]
    StringLitDouble(&'a str),
    #[regex(r#"'([^'\\]|\\.)*'"#)]
    StringLitSingle(&'a str),
    // to play nicer with the REPL, this will match until closing quotes or EOF
    #[token(r#"""""#, lex_triple_double_quote)]
    TripleDoubleQuote(&'a str),
    // to play nicer with the REPL, this will match until closing quotes or EOF
    #[token("'''", lex_triple_single_quote)]
    TripleSingleQuote(&'a str),
    #[regex(r"true")]
    True(&'a str),
    #[regex(r"false")]
    False(&'a str),
    #[regex(r"[ \t]+")]
    Whitespace(&'a str),
    #[regex(r"//([^-\n][^\n]*)?", priority = 2)] // needs to be matched before division
    LineComment(&'a str),
    #[regex(r"//-[^-]*(-+[^/-][^-]*)*-+//")]
    BlockComment(&'a str),
    #[regex(r"\+")]
    Plus,
    #[regex(r"-")]
    Minus,
    #[regex(r"\*")]
    Star,
    #[regex(r"/")]
    Slash,
    #[regex(r"%")]
    Percent,
    #[regex(r"\^")]
    Caret,
    #[regex(r"and")]
    And(&'a str),
    #[regex(r"or")]
    Or(&'a str),
    #[regex(r"not")]
    Not(&'a str),
    #[regex(r"xor")]
    Xor(&'a str),
    #[regex(r"==")]
    Eq(&'a str),
    #[regex(r"!=")]
    Neq(&'a str),
    #[regex(r"<")]
    Lt,
    #[regex(r">")]
    Gt,
    #[regex(r"<=")]
    Leq(&'a str),
    #[regex(r">=")]
    Geq(&'a str),
    #[regex(r"\$")]
    Dollar,
    #[regex(r"\(")]
    LParen,
    #[regex(r"\)")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("return")]
    Return(&'a str),
    #[regex(r"\\[ \t]*\n[ \t]*")]
    LineContinuation(&'a str),
    // helper token for the REPL to match end-of-input \
    #[regex(r"\\[ \t]*")]
    TrailingBackslash(&'a str),
    #[regex(r"\n[ \t]*")]
    Newline(&'a str),
    #[regex(r"=")]
    Assign,
    #[regex(r",")]
    Comma,
    #[regex(r"\|")]
    Pipe,
    #[regex(r"@")]
    At,
    #[regex(r"\?")]
    QuestionMark,
    #[regex(r"!")]
    ExclamationMark,
    #[regex(r"if")]
    If(&'a str),
    #[regex(r":")]
    Colon,
    #[regex(r"else")]
    Else(&'a str),
    #[regex(r"ask")]
    Ask(&'a str),
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("..")]
    DotDot,
    #[regex(r"del")]
    Del(&'a str),
    #[regex(r"iter")]
    Iter(&'a str),
    #[regex(r"set")]
    Set(&'a str),
    #[regex(r"list")]
    List(&'a str),
    #[regex(r"key")]
    Key(&'a str),
    #[regex(r"value")]
    Value(&'a str),
    #[regex(r"index")]
    Index(&'a str),
    // just to assert during tests that we matched the entire input
    #[allow(dead_code, non_camel_case_types)]
    __Test_Eof,
}

fn lex_triple_double_quote<'a>(lex: &mut Lexer<'a, Tok<'a>>) -> &'a str {
    let start = lex.span().start;
    let remainder = lex.remainder();

    // go to either closing quotes or eof
    if let Some(end_pos) = find_closing_triple_quote(remainder, b"\"\"\"") {
        lex.bump(end_pos + 3);
    } else {
        lex.bump(remainder.len());
    }
    &lex.source()[start..lex.span().end]
}

fn lex_triple_single_quote<'a>(lex: &mut Lexer<'a, Tok<'a>>) -> &'a str {
    let start = lex.span().start;
    let remainder = lex.remainder();

    // go to either closing quotes or eof
    if let Some(end_pos) = find_closing_triple_quote(remainder, b"'''") {
        lex.bump(end_pos + 3);
    } else {
        lex.bump(remainder.len());
    }
    &lex.source()[start..lex.span().end]
}

fn find_closing_triple_quote(s: &str, quote: &[u8; 3]) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // skip \" and \'
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if i + 2 < bytes.len() && &bytes[i..i + 3] == quote {
            return Some(i);
        }
        i += 1;
    }
    None
}

impl Tok<'_> {
    pub fn info(&self) -> String {
        match self {
            Tok::Num(value)
            | Tok::StringLitSingle(value)
            | Tok::StringLitDouble(value)
            | Tok::TripleSingleQuote(value)
            | Tok::TripleDoubleQuote(value)
            | Tok::TrailingBackslash(value)
            | Tok::Literal(value) => value,
            Tok::Whitespace(_) => "whitespace",
            Tok::LineComment(_) => "comment",
            Tok::BlockComment(_) => "multiline comment",
            Tok::True(_) => "true",
            Tok::False(_) => "false",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Percent => "%",
            Tok::Caret => "^",
            Tok::And(_) => "and",
            Tok::Or(_) => "or",
            Tok::Not(_) => "not",
            Tok::Xor(_) => "xor",
            Tok::Eq(_) => "==",
            Tok::Neq(_) => "!=",
            Tok::Lt => "<",
            Tok::Gt => ">",
            Tok::Leq(_) => "<=",
            Tok::Geq(_) => ">=",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::Dollar => "$",
            Tok::Assign => "=",
            Tok::Comma => ",",
            Tok::Pipe => "|",
            Tok::At => "@",
            Tok::__Test_Eof => "EOF",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::Return(_) => "return",
            Tok::QuestionMark => "?",
            Tok::ExclamationMark => "!",
            Tok::If(_) => "if",
            Tok::Else(_) => "else",
            Tok::Ask(_) => "ask",
            Tok::Colon => ":",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::DotDot => "..",
            Tok::Del(_) => "del",
            Tok::Iter(_) => "iter",
            Tok::Set(_) => "set",
            Tok::List(_) => "list",
            Tok::Key(_) => "key",
            Tok::Value(_) => "value",
            Tok::Index(_) => "index",
            Tok::LineContinuation(_) => "\\",
            Tok::Newline(_) => "\n",
        }
        .into()
    }

    pub fn len(&self) -> usize {
        match self {
            Tok::Num(slice)
            | Tok::Literal(slice)
            | Tok::True(slice)
            | Tok::False(slice)
            | Tok::StringLitSingle(slice)
            | Tok::StringLitDouble(slice)
            | Tok::TripleDoubleQuote(slice)
            | Tok::TripleSingleQuote(slice)
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
            | Tok::LineContinuation(slice)
            | Tok::Newline(slice)
            | Tok::Return(slice)
            | Tok::TrailingBackslash(slice)
            | Tok::If(slice)
            | Tok::Else(slice)
            | Tok::Ask(slice)
            | Tok::Del(slice)
            | Tok::Iter(slice)
            | Tok::Set(slice)
            | Tok::List(slice)
            | Tok::Key(slice)
            | Tok::Value(slice)
            | Tok::Index(slice)
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
            | Tok::LParen
            | Tok::RParen
            | Tok::LBrace
            | Tok::RBrace
            | Tok::Assign
            | Tok::Comma
            | Tok::Pipe
            | Tok::QuestionMark
            | Tok::ExclamationMark
            | Tok::Colon
            | Tok::DotDot
            | Tok::LBracket
            | Tok::RBracket
            | Tok::At => 1,
            Tok::__Test_Eof => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tok::*;

    pub fn expect_tokens(input: &str, expected: &[Tok]) {
        let mut tokens = Tok::lexer(input);
        for (i, expected_tok) in expected.iter().enumerate() {
            let maybe_lexed = tokens
                .next()
                .transpose()
                .expect(format!("[{i}] lexing error").as_str());
            match (expected_tok, maybe_lexed) {
                (__Test_Eof, None) => break,
                (__Test_Eof, Some(tok)) => {
                    panic!("[{i}] Expected to have finished lexing, but got {tok:?}")
                }
                (_, None) => panic!("[{i}] Expected {expected_tok:?}, but got EOF"),
                (_, Some(actual)) => assert_eq!(
                    expected_tok, &actual,
                    "Token mismatch at index {}: expected {:?}, got {:?}",
                    i, expected_tok, actual
                ),
            }
        }
    }

    #[test]
    fn test_arithmetic() {
        expect_tokens(
            "1 + 2.3    ",
            &[
                Num("1"),
                Whitespace(" "),
                Plus,
                Whitespace(" "),
                Num("2.3"),
                Whitespace(" ".repeat(4).as_str()),
                __Test_Eof,
            ],
        );
        expect_tokens("1_000_000", &[Num("1_000_000"), __Test_Eof]);
        expect_tokens(
            "+-*/%^",
            &[Plus, Minus, Star, Slash, Percent, Caret, __Test_Eof],
        );
    }

    #[test]
    fn test_comments() {
        expect_tokens(
            r#"1//normal comment
2//-multiline on single line-//3
4//-multiline
on multiple
lines-//5
6"#,
            &[
                Num("1"),
                LineComment("//normal comment"),
                Newline("\n"),
                Num("2"),
                BlockComment("//-multiline on single line-//"),
                Num("3"),
                Newline("\n"),
                Num("4"),
                BlockComment("//-multiline\non multiple\nlines-//"),
                Num("5"),
                Newline("\n"),
                Num("6"),
                __Test_Eof,
            ],
        )
    }

    #[test]
    fn test_others() {
        expect_tokens(
            "()$=,|@{ x }!? if else:",
            &[
                LParen,
                RParen,
                Dollar,
                Assign,
                Comma,
                Pipe,
                At,
                LBrace,
                Whitespace(" "),
                Literal("x"),
                Whitespace(" "),
                RBrace,
                ExclamationMark,
                QuestionMark,
                Whitespace(" "),
                If("if"),
                Whitespace(" "),
                Else("else"),
                Colon,
                __Test_Eof,
            ],
        );
    }

    #[test]
    fn test_booleans() {
        expect_tokens(
            "true false",
            &[True("true"), Whitespace(" "), False("false"), __Test_Eof],
        );
        expect_tokens(
            "and not or xor < > <= >=",
            &[
                And("and"),
                Whitespace(" "),
                Not("not"),
                Whitespace(" "),
                Or("or"),
                Whitespace(" "),
                Xor("xor"),
                Whitespace(" "),
                Lt,
                Whitespace(" "),
                Gt,
                Whitespace(" "),
                Leq("<="),
                Whitespace(" "),
                Geq(">="),
                __Test_Eof,
            ],
        );
        // keyword match is exact
        expect_tokens(
            "organic andnot",
            &[
                Literal("organic"),
                Whitespace(" "),
                Literal("andnot"),
                __Test_Eof,
            ],
        );
    }

    #[test]
    fn test_newline() {
        expect_tokens("1\n2", &[Num("1"), Newline("\n"), Num("2"), __Test_Eof]);
    }

    #[test]
    fn test_newline_with_indent() {
        expect_tokens(
            "1\n    2",
            &[Num("1"), Newline("\n    "), Num("2"), __Test_Eof],
        );
    }

    #[test]
    fn test_line_continuation() {
        expect_tokens(
            "1 +\\\n    2",
            &[
                Num("1"),
                Whitespace(" "),
                Plus,
                LineContinuation("\\\n    "),
                Num("2"),
                __Test_Eof,
            ],
        );
    }

    #[test]
    fn test_line_continuation_before_newline() {
        // 1\n    + 2 with continuation
        expect_tokens(
            "1\\\n    + 2",
            &[
                Num("1"),
                LineContinuation("\\\n    "),
                Plus,
                Whitespace(" "),
                Num("2"),
                __Test_Eof,
            ],
        );
    }

    #[test]
    fn test_return_keyword() {
        expect_tokens(
            "return 42",
            &[Return("return"), Whitespace(" "), Num("42"), __Test_Eof],
        );
    }

    #[test]
    fn test_string_double_quotes() {
        expect_tokens(r#""hello""#, &[StringLitDouble(r#""hello""#), __Test_Eof]);
    }

    #[test]
    fn test_string_single_quotes() {
        expect_tokens(r#"'hello'"#, &[StringLitSingle(r#"'hello'"#), __Test_Eof]);
    }

    #[test]
    fn test_string_with_interpolation_braces() {
        // Lexer just captures the whole string, braces and all
        expect_tokens(
            r#""hello {name}""#,
            &[StringLitDouble(r#""hello {name}""#), __Test_Eof],
        );
    }

    #[test]
    fn test_nested_quotes() {
        // Double quotes containing single quotes in interpolation
        expect_tokens(
            r#""hello {'world'}""#,
            &[StringLitDouble(r#""hello {'world'}""#), __Test_Eof],
        );
    }
    #[test]
    fn test_multiline_closed() {
        // Double quotes containing single quotes in interpolation
        expect_tokens(
            r#""""hello\#
world""""#,
            &[TripleDoubleQuote("\"\"\"hello\\#\nworld\"\"\""), __Test_Eof],
        );
    }

    #[test]
    fn test_multiline_open() {
        // Double quotes containing single quotes in interpolation
        expect_tokens(
            r#""""hello\#
world"#,
            &[TripleDoubleQuote("\"\"\"hello\\#\nworld"), __Test_Eof],
        );
    }

    #[test]
    fn test_trailing_backslash() {
        expect_tokens(
            r#"3 +\"#,
            &[
                Num("3"),
                Whitespace(" "),
                Plus,
                TrailingBackslash("\\"),
                __Test_Eof,
            ],
        );
    }

    #[test]
    fn test_fn_application_without_spaces() {
        expect_tokens(
            "3@fn()",
            &[Num("3"), At, Literal("fn"), LParen, RParen, __Test_Eof],
        );
    }
}
