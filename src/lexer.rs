use logos::Logos;

#[derive(Logos, Debug, PartialEq, Copy, Clone)]
pub enum Tok<'a> { // to make it easier to access token len, all tokens that are bigger than 1 char hold their slice, even if static
    #[regex(r"[a-zA-Z][a-zA-Z_]*")]
    Literal(&'a str),
    //values
    #[regex(r"[0-9_]+(\.[0-9_]+)?")]
    Num(&'a str),
    #[regex(r"true")]
    True(&'a str),
    #[regex(r"false")]
    False(&'a str),
    // trivia
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
    // grouping
    #[regex(r"\(")]
    LParen,
    #[regex(r"\)")]
    RParen,
    #[allow(dead_code, non_camel_case_types)]
    __Test_Eof,
}

impl Tok<'_> {
    pub fn info(&self) -> String {
        match self {
            Tok::Num(value) |
            Tok::Literal(value) => value,
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
            Tok::__Test_Eof => "EOF",
        }
        .into()
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

    #[test]
    fn test_grouping() {
        expect_tokens("()", &[LParen, RParen]);
    }

    #[test]
    fn test_booleans() {
        expect_tokens("true false", &[True("true"), Whitespace(" "), False("false"), __Test_Eof]);
        expect_tokens("and not or xor < > <= >=", &[And("and"), Whitespace(" "), Not("not"), Whitespace(" "), Or("or"), Whitespace(" "), Xor("xor"), Whitespace(" "), Lt, Whitespace(" "), Gt, Whitespace(" "), Leq("<="), Whitespace(" "), Geq(">="), __Test_Eof]);
        // keyword match is exact
        expect_tokens("organic andnot", &[Literal("organic"), Whitespace(" "), Literal("andnot"), __Test_Eof]);
    }
}
