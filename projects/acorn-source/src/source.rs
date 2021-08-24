use crate::capabilities::SourceCapabilities;
use crate::identity::SourceIdentity;
use crate::read::{ReadOutcome, ReadResult, SourceRange};

/// Random-access byte source contract.
pub trait ByteSource {
    /// Returns the total length when known.
    fn length(&self) -> Option<u64>;

    /// Reads up to `destination.len()` bytes starting at `offset`.
    ///
    /// Implementations must not write past the returned byte count and must not
    /// zero-fill unread portions of `destination`.
    fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult;

    /// Reads exactly the requested range into a fresh buffer.
    fn read_range(&self, range: SourceRange) -> Result<Vec<u8>, ReadError> {
        let mut buffer = vec![0u8; range.len as usize];
        match self.read_at(range.start, &mut buffer)? {
            ReadOutcome::Complete => Ok(buffer),
            ReadOutcome::Partial { bytes_read } => {
                buffer.truncate(bytes_read as usize);
                Err(ReadError::ShortRead {
                    expected: range.len,
                    actual: bytes_read,
                })
            }
            ReadOutcome::Eof => Err(ReadError::Eof),
            ReadOutcome::Unavailable { range, reason } => {
                Err(ReadError::Unavailable { range, reason })
            }
        }
    }

    /// Returns source capabilities.
    fn capabilities(&self) -> SourceCapabilities;

    /// Returns a stable identity for caching.
    fn identity(&self) -> SourceIdentity;
}

/// Errors from `read_range` convenience reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(String),
    /// Read ended before the requested number of bytes.
    #[error("short read: expected {expected} bytes, got {actual}")]
    ShortRead {
        /// Requested byte count.
        expected: u64,
        /// Bytes actually read.
        actual: u64,
    },
    /// Read started at or past EOF.
    #[error("unexpected eof")]
    Eof,
    /// Requested interval is not currently available.
    #[error("unavailable range: {reason}")]
    Unavailable {
        /// Missing interval.
        range: SourceRange,
        /// Why the interval is unavailable.
        reason: String,
    },
}

impl From<std::io::Error> for ReadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
