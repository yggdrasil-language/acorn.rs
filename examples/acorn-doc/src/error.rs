use std::fmt;

/// Errors while reading legacy Word documents.
#[derive(Debug)]
pub enum DocError {
    /// Input is not a compound file.
    NotCompoundFile,
    /// Compound file container could not be opened.
    CompoundFile(String),
    /// Required OLE stream is missing or unreadable.
    Stream {
        /// Stream path such as `/WordDocument`.
        stream: String,
        /// Underlying read error.
        message: String,
    },
    /// Document uses unsupported protection.
    Encrypted,
    /// FIB layout or text range is invalid.
    InvalidFib,
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCompoundFile => write!(f, "input is not an OLE Compound File"),
            Self::CompoundFile(message) => write!(f, "compound file: {message}"),
            Self::Stream { stream, message } => write!(f, "{stream} stream: {message}"),
            Self::Encrypted => write!(f, "encrypted or obfuscated Word documents are not supported"),
            Self::InvalidFib => write!(f, "invalid Word FIB or text range"),
        }
    }
}

impl std::error::Error for DocError {}

/// Result alias for legacy DOC operations.
pub type Result<T> = std::result::Result<T, DocError>;
