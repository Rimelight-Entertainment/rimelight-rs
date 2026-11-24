//! Errors for import, export, drafts and the editor API.

/// Errors from this crate. Page JSON that is merely unfamiliar (unknown block types, unknown props) is stored
/// losslessly and never produces an error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The page JSON does not have the expected shape (missing `meta`, `content`, a block `id`, and so on).
    #[error("invalid page JSON: {0}")]
    Shape(String),
    /// An inline node of a type this crate does not know, found where rich text must be stored.
    #[error("unknown inline node type: {0}")]
    UnknownInline(String),
    /// A Loro call failed.
    #[error("loro error: {0}")]
    Loro(String),
    /// The block is templated (`isTemplated`) and the operation deletes or moves it, or sets a locked prop.
    #[error("locked by template")]
    Locked,
    /// The operation would break a registry rule (nesting, a text field, a web-only block, and so on).
    #[error("not allowed: {0}")]
    NotAllowed(String),
    /// A block, field or row that does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// A byte offset or row index outside the value, or not on a character boundary.
    #[error("invalid range: {0}")]
    InvalidRange(String),
    /// A sync frame that cannot be decoded.
    #[error("invalid sync frame: {0}")]
    Protocol(String),
}

/// Turns any Loro error into [`Error::Loro`].
pub(crate) fn map_loro<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, Error> {
    result.map_err(|e| Error::Loro(e.to_string()))
}
