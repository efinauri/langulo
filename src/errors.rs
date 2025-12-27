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
    InternalError {
        message: String,
    },

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

    #[error("Lexer error at position {position}")]
    #[diagnostic(
        code(languria::lexer::no_match),
        help("No lexer rule matches this input")
    )]
    LexerError {
        position: usize,
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
    PythonError {
        message: String,
    },

    #[error("Transpilation error: {message}")]
    #[diagnostic(code(languria::transpile::error))]
    TranspileError {
        message: String,
    },
}

impl LanguriaError {
    pub fn unexpected_eof(src: &str) -> Self {
        let len = src.len();
        Self::UnexpectedEOF {
            src: src.to_string(),
            span: (len.saturating_sub(1), 1).into(),
        }
    }

    pub fn unexpected_token(src: &str, token: &str, offset: usize) -> Self {
        Self::UnexpectedToken {
            token: token.to_string(),
            src: src.to_string(),
            span: (offset, token.len().max(1)).into(),
        }
    }

    pub fn lexer_error(src: &str, position: usize) -> Self {
        Self::LexerError {
            position,
            src: src.to_string(),
            span: (position, 1).into(),
        }
    }

    pub fn invalid_number(src: &str, value: &str, offset: usize) -> Self {
        Self::InvalidNumber {
            value: value.to_string(),
            src: src.to_string(),
            span: (offset, value.len()).into(),
        }
    }
}

pub type LanguriaResult<T> = Result<T, LanguriaError>;

/// For backward compatibility during migration
impl From<&str> for LanguriaError {
    fn from(msg: &str) -> Self {
        LanguriaError::InternalError {
            message: msg.to_string(),
        }
    }
}