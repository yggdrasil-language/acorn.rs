use thiserror::Error;

/// A half-open byte interval for source reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceRange {
    /// Inclusive start offset.
    pub start: u64,
    /// Length in bytes.
    pub len: u64,
}

/// Errors from checked source range arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RangeError {
    /// `start + len` overflowed `u64`.
    #[error("source range overflow")]
    Overflow,
    /// Range end exceeds the known source length.
    #[error("source range out of bounds")]
    OutOfBounds,
}

impl SourceRange {
    /// Creates a range after validating `start + len`.
    pub fn new(start: u64, len: u64) -> Result<Self, RangeError> {
        let end = start.checked_add(len).ok_or(RangeError::Overflow)?;
        if end < start {
            return Err(RangeError::Overflow);
        }
        Ok(Self { start, len })
    }

    /// Returns the exclusive end offset.
    pub fn end(self) -> Result<u64, RangeError> {
        self.start.checked_add(self.len).ok_or(RangeError::Overflow)
    }

    /// Returns whether the range fits in `[0, source_len)`.
    pub fn fits_in(self, source_len: u64) -> Result<(), RangeError> {
        let end = self.end()?;
        if end > source_len {
            return Err(RangeError::OutOfBounds);
        }
        Ok(())
    }
}

/// Outcome of a bounded read attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    /// All requested bytes were read.
    Complete,
    /// Fewer bytes than requested were available before EOF.
    Partial {
        /// Number of bytes written into the destination buffer.
        bytes_read: u64,
    },
    /// The read starts at or past EOF.
    Eof,
    /// The requested interval is not currently available.
    Unavailable {
        /// Missing interval.
        range: SourceRange,
        /// Why the interval is unavailable.
        reason: String,
    },
}

/// Result of reading into a caller-provided buffer.
pub type ReadResult = Result<ReadOutcome, std::io::Error>;
