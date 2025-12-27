use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Rich error type with source locations for beautiful error reporting
#[derive(Error, Debug, Diagnostic)]
pub enum LanguriaError {
    #[error("Internal compiler error: {message}")]
    #[diagnostic(
        code(languria::internal),
        help("This is a bug in Languria. Please report it!")
    )]
    InternalError { message: String },

    #[error("Unexpected end of input")]
    #[diagnostic(
        code(languria::syntax::unexpected_eof),
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
        code(languria::syntax::unexpected_token),
        help("Expected a number or operator here")
    )]
    UnexpectedToken {
        token: String,
        #[source_code]
        src: String,
        #[label("unexpected token")]
        span: SourceSpan,
    },

    #[error("Invalid number format: '{value}'")]
    #[diagnostic(
        code(languria::syntax::invalid_number),
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
        code(languria::lexer::no_match),
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
        code(languria::runtime::python),
        help("The generated Python code failed to execute")
    )]
    PythonError { message: String },

    #[error("Transpilation error: {message}")]
    #[diagnostic(code(languria::transpile::error))]
    TranspileError { message: String },

    #[error("Expected '{expected}' but found '{found}'")]
    #[diagnostic(
        code(languria::syntax::unexpected_token),
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
}

pub type LanguriaResult<T> = Result<T, LanguriaError>;
