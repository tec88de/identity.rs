use anyhow::Result as AnyhowResult;
use thiserror::Error as DeriveError;

/// The main crate Error type; uses `thiserror`.
#[derive(Debug, DeriveError)]
pub enum Error {
    /// Error identity_core
    #[error("Identity_core Error: {0}")]
    IdentityCoreError(#[from] identity_core::Error),
}

/// The main crate result type derived from the `anyhow::Result` type.
pub type Result<T> = AnyhowResult<T, Error>;