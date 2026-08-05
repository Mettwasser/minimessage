pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error, PartialEq, Clone)]
pub enum Error {
    #[error("Invalid Token: {_0:?}")]
    InvalidToken(String),

    #[error("Unexpected EOF")]
    UnexpectedEof,

    #[error("Mismatched tag: expected </{expected}> but found </{found}>")]
    MismatchedTag { expected: String, found: String },
}
