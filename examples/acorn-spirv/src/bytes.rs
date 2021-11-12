//! Byte-slice validation entry that needs no caller-built source or ledger.

use std::fmt::{Display, Formatter};

use acorn_core::{Budget, BudgetError, BudgetTracker};
use acorn_source::{MemorySource, ReadError};

use crate::{SpirvLayoutError, SpirvSummary, validate_spirv};

/// Failure from [`validate_spirv_bytes`].
///
/// Each variant keeps the original typed error, so nothing is flattened into a
/// message string. Budget exhaustion is separated from structural failure even
/// though the layout pass reports it through [`SpirvLayoutError::Budget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpirvByteError {
    /// The module failed structural validation.
    Layout(SpirvLayoutError),
    /// The read allowance was exhausted.
    Budget(BudgetError),
    /// The backing source could not provide bytes.
    Source(ReadError),
}

impl Display for SpirvByteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout(error) => write!(f, "SPIR-V module could not be mapped: {error}"),
            Self::Budget(error) => write!(f, "SPIR-V read budget exhausted: {error}"),
            Self::Source(error) => write!(f, "SPIR-V byte source failed: {error}"),
        }
    }
}

impl std::error::Error for SpirvByteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Budget(error) => Some(error),
            Self::Source(error) => Some(error),
        }
    }
}

impl From<SpirvLayoutError> for SpirvByteError {
    fn from(error: SpirvLayoutError) -> Self {
        match error {
            SpirvLayoutError::Budget(error) => Self::Budget(error),
            other => Self::Layout(other),
        }
    }
}

impl From<BudgetError> for SpirvByteError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

impl From<ReadError> for SpirvByteError {
    fn from(error: ReadError) -> Self {
        Self::Source(error)
    }
}

/// Validates a complete SPIR-V byte slice without exposing source or ledger types.
///
/// The slice is wrapped in an in-memory source and validated under a default
/// budget, so a caller only needs `&[u8]`. Structural failures, budget exhaustion
/// and source failures stay typed in [`SpirvByteError`].
pub fn validate_spirv_bytes(bytes: &[u8]) -> Result<SpirvSummary, SpirvByteError> {
    let source = MemorySource::from_bytes("spirv-bytes", bytes.to_vec());
    let mut ledger = BudgetTracker::new(Budget::default());
    validate_spirv(&source, &mut ledger).map_err(SpirvByteError::from)
}
