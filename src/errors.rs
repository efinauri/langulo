#[derive(Debug)]
pub enum LanguriaError {
    /// a bug, not the user's fault
    InternalError(String),
    SemanticUnexpectedEOF,
    UnexpectedToken(String),
}

pub type LanguriaResult<T> = Result<T, LanguriaError>;