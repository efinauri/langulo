use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

// values all start with underscore because of
// https://github.com/zkat/miette/issues/458

#[derive(Error, Debug, Diagnostic)]
pub enum LanguloError {
    #[error("Internal error: {_message}")]
    #[diagnostic(
        code(langulo::internal),
        help("This is a bug in Langulo. Please report it!")
    )]
    InternalError {
        _message: String,
        // #[source_code]
        // _src: String,
        // #[label("here")]
        // _span: SourceSpan,
    },

    #[error("Unexpected end of input")]
    #[diagnostic(code(langulo::syntax::unexpected_eof))]
    UnexpectedEOF {
        #[source_code]
        _src: String,
        #[label("unterminated expression")]
        _span: SourceSpan,
    },

    #[error("Unexpected token: '{_token}'.")]
    #[diagnostic(
        code(langulo::syntax::unexpected_token),
        help("Was expecting: '{_expected}'")
    )]
    UnexpectedToken {
        _token: String,
        _expected: String,
        #[source_code]
        _src: String,
        #[label("unexpected token")]
        _span: SourceSpan,
    },

    #[error("Invalid number format: '{_value}'")]
    #[diagnostic(
        code(langulo::syntax::invalid_number),
        help(
            "You cannot omit digits before or after a floating point. `.3` or `-1.` are invalid numbers."
        )
    )]
    InvalidNumber {
        _value: String,
        #[source_code]
        _src: String,
        #[label("invalid number")]
        _span: SourceSpan,
    },

    #[error("Lexer error")]
    #[diagnostic(
        code(langulo::lexer::no_match),
        help("No lexer rule matches this input")
    )]
    LexerError {
        #[source_code]
        _src: String,
        #[label("unrecognized character")]
        _span: SourceSpan,
    },

    #[error("Python execution error: {_message}")]
    #[diagnostic(
        code(langulo::runtime::python),
        help("The generated Python code failed to execute")
    )]
    PythonError { _message: String },

    #[error("Invalid assignment target")]
    #[diagnostic(code(langulo::transpile::invalid_lvalue))]
    InvalidAssignmentTarget {
        #[source_code]
        _src: String,
        #[label("cannot be assigned to")]
        _span: SourceSpan,
    },

    #[error("Empty grouping expression")]
    #[diagnostic(
        code(langulo::transpile::empty_grouping),
        help("grouping statements need at least one expression to evaluate to")
    )]
    EmptyBlock {
        #[source_code]
        _src: String,
        #[label("here")]
        _span: SourceSpan,
    },

    #[error("Unterminated interpolation")]
    #[diagnostic(
        code(langulo::syntax::interpolation),
        help("If you meant to use `{{` as a character, escape it: `\\{{`")
    )]
    UnterminatedInterpolation {
        #[source_code]
        _src: String,
        #[label("here")]
        _span: SourceSpan,
    },
    #[error("Inside a string interpolation, expressions using braces are not allowed")]
    #[diagnostic(
        code(langulo::syntax::interpolation),
        help("Consider assigning this expression to a variable and interpolate that instead")
    )]
    LBraceInsideStringInterpolation {
        #[source_code]
        _src: String,
        #[label("statement using braces")]
        _span: SourceSpan,
    },
    
    #[error("Return expressions are only allowed inside grouping statements")]
    #[diagnostic(
        code(langulo::transpile::return_outside_block),
    )]
    ReturnOutsideBlock {
        #[source_code]
        _src: String,
        #[label("here")]
        _span: SourceSpan,
    },
}

pub type LanguloResult<T> = Result<T, LanguloError>;
