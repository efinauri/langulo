use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Rich error type with source locations for beautiful error reporting
#[derive(Error, Debug, Diagnostic)]
pub enum LanguloError {
    #[error("Internal compiler error: {message}")]
    #[diagnostic(
        code(langulo::internal),
        help("This is a bug in Langulo. Please report it!")
    )]
    InternalError { message: String },

    #[error("Unexpected end of input")]
    #[diagnostic(
        code(langulo::syntax::unexpected_eof),
        help("The expression appears to be incomplete. Did you forget an operand?")
    )]
    UnexpectedEOF {
        #[source_code]
        src: String,
        #[label("input ends here")]
        span: SourceSpan,
    },

    #[error("Unexpected token: '{token}'")]
    #[diagnostic(
        code(langulo::syntax::unexpected_token),
        help("Expected a number or operator here")
    )]
    UnexpectedToken {
        token: String,
        #[source_code]
        src: String,
        #[label("unexpected token")]
        span: SourceSpan,
    },

    #[error("Unexpected token: '{token}'.")]
    #[diagnostic(
        code(langulo::syntax::unexpected_token),
        help("Was expecting: '{expected}'")
    )]
    UnexpectedTokenWasExpecting {
        token: String,
        expected: String,
        #[source_code]
        src: String,
        #[label("unexpected token")]
        span: SourceSpan,
    },

    #[error("Invalid number format: '{value}'")]
    #[diagnostic(
        code(langulo::syntax::invalid_number),
        help("Numbers should be in the format: 123, 1_000, or 3.14")
    )]
    InvalidNumber {
        value: String,
        #[source_code]
        src: String,
        #[label("invalid number")]
        span: SourceSpan,
    },

    #[error("Lexer error")]
    #[diagnostic(
        code(langulo::lexer::no_match),
        help("No lexer rule matches this input")
    )]
    LexerError {
        #[source_code]
        src: String,
        #[label("unrecognized character")]
        span: SourceSpan,
    },

    #[error("Python execution error: {message}")]
    #[diagnostic(
        code(langulo::runtime::python),
        help("The generated Python code failed to execute")
    )]
    PythonError { message: String },

    #[error("Transpilation error: {message}")]
    #[diagnostic(code(langulo::transpile::error))]
    TranspileError { message: String },

    #[error("Expected '{expected}' but found '{found}'")]
    #[diagnostic(
        code(langulo::syntax::unexpected_token),
        help("Expected a number or operator here")
    )]
    ExpectedTokenAbsent {
        expected: String,
        found: String,
        #[source_code]
        src: String,
        #[label("unexpected token")]
        span: SourceSpan,
    },
    #[error("Empty blocks are not allowed")]
    #[diagnostic(code(langulo::syntax::empty_block))]
    EmptyBlock {
        #[source_code]
        src: String,
        #[label("empty blocks are not allowed")]
        span: SourceSpan,
    },
    #[error("Unclosed string literal.")]
    #[diagnostic(code(langulo::syntax::empty_block))]
    UnclosedString {
        #[source_code]
        src: String,
        #[label("empty blocks are not allowed")]
        span: SourceSpan,
    },
    #[error("Unterminated interpolation.")]
    #[diagnostic(
        code(langulo::syntax::interpolation),
        help("If you want to use a left brace as a character, escape it: \\{{")
    )]
    UnterminatedInterpolation {
        #[source_code]
        src: String,
        #[label("unterminated interpolation")]
        span: SourceSpan,
    },
    #[error("Inside a string interpolation, statements using braces are not allowed")]
    #[diagnostic(
        code(langulo::syntax::interpolation),
        help("assign this statement to a variable and interpolate that instead")
    )]
    LBraceInsideStringInterpolation {
        #[source_code]
        src: String,
        #[label("statement using braces")]
        span: SourceSpan,
    },
}

pub type LanguloResult<T> = Result<T, LanguloError>;
