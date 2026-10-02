use crate::sink::{ByteSink, WriteError};
use crate::sink_capabilities::SinkCapabilities;
use crate::write::WriteOutcome;

/// In-memory `ByteSink` with seek, append, and patch support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySink {
    bytes: Vec<u8>,
    budget: Option<u64>,
}

impl MemorySink {
    /// Creates an empty sink with no byte budget.
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            budget: None,
        }
    }

    /// Creates an empty sink with a maximum byte budget.
    pub fn with_budget(budget: u64) -> Self {
        Self {
            bytes: Vec::new(),
            budget: Some(budget),
        }
    }

    /// Returns the written bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Returns a view of the written bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn check_budget(&self, end: u64) -> Result<(), WriteError> {
        if let Some(limit) = self.budget {
            if end > limit {
                return Err(WriteError::BudgetExceeded {
                    attempted: end,
                    limit,
                });
            }
        }
        Ok(())
    }

    fn ensure_capacity(&mut self, end: u64) -> Result<(), WriteError> {
        self.check_budget(end)?;
        let end = end as usize;
        if self.bytes.len() < end {
            self.bytes.resize(end, 0);
        }
        Ok(())
    }
}

impl Default for MemorySink {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteSink for MemorySink {
    fn capabilities(&self) -> SinkCapabilities {
        SinkCapabilities::MEMORY
    }

    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn append(&mut self, bytes: &[u8]) -> Result<WriteOutcome, WriteError> {
        let end = self.bytes.len() as u64 + bytes.len() as u64;
        if let Some(limit) = self.budget {
            if end > limit {
                return Ok(WriteOutcome::WouldExceedBudget {
                    attempted: end,
                    limit,
                });
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(WriteOutcome::Complete)
    }

    fn write_at_impl(&mut self, offset: u64, bytes: &[u8]) -> Result<WriteOutcome, WriteError> {
        let end = offset + bytes.len() as u64;
        self.ensure_capacity(end)?;
        let start = offset as usize;
        self.bytes[start..start + bytes.len()].copy_from_slice(bytes);
        if self.bytes.len() < end as usize {
            self.bytes.truncate(end as usize);
        }
        Ok(WriteOutcome::Complete)
    }
}
