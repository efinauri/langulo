use logos::Logos;

#[derive(Logos, Debug, PartialEq, Copy, Clone)]
pub enum Tok<'a> {
    // only numbers and numeric operations for now
    #[regex(r"[0-9_]+(\.[0-9_]+)?")]
    Num(&'a str),
    #[regex(r"[ \t\n\r]+")]
    Whitespace(&'a str),
    #[regex(r"//([^-\n][^\n]*)?", priority = 2)] // needs to be matched before division
    LineComment(&'a str),
    #[regex(r"//-[^-]*(-+[^/-][^-]*)*-+//")]
    BlockComment(&'a str),
    // operations
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
    #[allow(dead_code, non_camel_case_types)]
    __Test_Eof,
}

impl Tok<'_> {
    pub fn info(&self) -> String {
        match self {
            Tok::Num(value) => value,
            Tok::Whitespace(_) => "whitespace",
            Tok::LineComment(_) => "comment",
            Tok::BlockComment(_) => "multiline comment",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Percent => "%",
            Tok::Caret => "^",
            Tok::__Test_Eof => "EOF",
        }
        .to_string()
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
        expect_tokens("1 + 2.3    ", &[
            Num("1"), Whitespace(" "),
            Plus, Whitespace(" "),
            Num("2.3"), Whitespace(" ".repeat(4).as_str()),
            __Test_Eof]);
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
            &[Num("1"), LineComment("//normal comment"), Whitespace("\n"),
            Num("2"), BlockComment("//-multiline on single line-//"), Num("3"), Whitespace("\n"),
                Num("4"), BlockComment("//-multiline\non multiple\nlines-//"), Num("5"), Whitespace("\n"),
                Num("6"), __Test_Eof]
        )
    }
}
