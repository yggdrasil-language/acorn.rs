use thiserror::Error;

/// Errors while reading legacy Word documents.
#[derive(Debug, Error)]
pub enum DocError {
    /// Input is not a compound file.
    #[error("input is not an OLE Compound File")]
    NotCompoundFile,
    /// Compound file container could not be opened.
    #[error("compound file: {0}")]
    CompoundFile(String),
    /// Required OLE stream is missing or unreadable.
    #[error("{stream} stream: {message}")]
    Stream {
        /// Stream path such as `/WordDocument`.
        stream: String,
        /// Underlying read error.
        message: String,
    },
    /// Document uses unsupported protection.
    #[error("encrypted or obfuscated Word documents are not supported")]
    Encrypted,
    /// FIB layout or text range is invalid.
    #[error("invalid Word FIB or text range")]
    InvalidFib,
}

/// Result alias for legacy DOC operations.
pub type Result<T> = std::result::Result<T, DocError>;
