//! Errors for import, export and drafts.

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
}

/// Turns any Loro error into [`Error::Loro`].
pub(crate) fn map_loro<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, Error> {
    result.map_err(|e| Error::Loro(e.to_string()))
}
