use crate::directives::DirectiveConflict;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// A node does not have the structure its tag promises (a provider or grammar bug).
    #[error("unexpected syntax shape: {0}")]
    Shape(String),
    #[error(transparent)]
    Directives(#[from] Box<DirectiveConflict>),
    /// The safety check failed: the output is not valid, or means something else.
    #[error("internal error, file not changed: {0}")]
    Safety(String),
}
