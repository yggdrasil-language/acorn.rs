use crate::sink_capabilities::SinkCapabilities;
use crate::write::WriteOutcome;

/// Bounded byte sink contract, symmetric to `ByteSource` but for generation.
pub trait ByteSink {
    /// Returns sink capabilities.
    fn capabilities(&self) -> SinkCapabilities;

    /// Returns the number of bytes currently written.
    fn len(&self) -> u64;

    /// Appends bytes at the current end.
    fn append(&mut self, bytes: &[u8]) -> Result<WriteOutcome, WriteError>;

    /// Writes bytes at `offset`, growing the sink when supported.
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> Result<WriteOutcome, WriteError> {
        if !self.capabilities().seek_write {
            return Err(WriteError::Unsupported {
                operation: "write_at",
            });
        }
        self.write_at_impl(offset, bytes)
    }

    /// Patches a little-endian `u32` at `offset`.
    fn patch_u32_le(&mut self, offset: u64, value: u32) -> Result<(), WriteError> {
        if !self.capabilities().patch_in_place {
            return Err(WriteError::Unsupported {
                operation: "patch_u32_le",
            });
        }
        self.write_at(offset, &value.to_le_bytes())?;
        Ok(())
    }

    /// Implementation hook for sinks that support seek writes.
    fn write_at_impl(&mut self, offset: u64, bytes: &[u8]) -> Result<WriteOutcome, WriteError>;
}

/// Errors from sink writes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(String),
    /// The sink does not support the requested operation.
    #[error("unsupported sink operation: {operation}")]
    Unsupported {
        /// Operation name.
        operation: &'static str,
    },
    /// Offset is out of range for a non-growable sink.
    #[error("write offset {offset} is out of range for length {len}")]
    OutOfRange {
        /// Requested offset.
        offset: u64,
        /// Current sink length.
        len: u64,
    },
    /// Write would exceed the configured byte budget.
    #[error("write would exceed byte budget: attempted {attempted}, limit {limit}")]
    BudgetExceeded {
        /// Bytes that would be written.
        attempted: u64,
        /// Active byte budget.
        limit: u64,
    },
}

impl From<std::io::Error> for WriteError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
