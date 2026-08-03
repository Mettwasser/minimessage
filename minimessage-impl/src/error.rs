use crate::token::TokenOwned;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid Token: {_0:?}")]
    InvalidToken(TokenOwned),

    #[error("Unexpected EOF")]
    UnexpectedEof,

    #[error("Mismatched tag: expected </{expected}> but found </{found}>")]
    MismatchedTag { expected: String, found: String },
}
