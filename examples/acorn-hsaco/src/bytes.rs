//! Byte-slice validation entry that needs no caller-built source or ledger.

use std::fmt::{Display, Formatter};

use acorn_core::{Budget, BudgetError, BudgetTracker};
use acorn_source::{MemorySource, ReadError};

use crate::{HsacoLayoutError, HsacoSummary, validate_hsaco};

/// Failure from [`validate_hsaco_bytes`].
///
/// Each variant keeps the original typed error, so nothing is flattened into a
/// message string. Budget exhaustion is separated from structural failure even
/// though the layout pass reports it through [`HsacoLayoutError::Budget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HsacoByteError {
    /// The code object failed structural validation.
    Layout(HsacoLayoutError),
    /// The read allowance was exhausted.
    Budget(BudgetError),
    /// The backing source could not provide bytes.
    Source(ReadError),
}

impl Display for HsacoByteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout(error) => write!(f, "HSACO code object could not be mapped: {error}"),
            Self::Budget(error) => write!(f, "HSACO read budget exhausted: {error}"),
            Self::Source(error) => write!(f, "HSACO byte source failed: {error}"),
        }
    }
}

impl std::error::Error for HsacoByteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Budget(error) => Some(error),
            Self::Source(error) => Some(error),
        }
    }
}

impl From<HsacoLayoutError> for HsacoByteError {
    fn from(error: HsacoLayoutError) -> Self {
        match error {
            HsacoLayoutError::Budget(error) => Self::Budget(error),
            other => Self::Layout(other),
        }
    }
}

impl From<BudgetError> for HsacoByteError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

impl From<ReadError> for HsacoByteError {
    fn from(error: ReadError) -> Self {
        Self::Source(error)
    }
}

/// Validates a complete ELF64 code object byte slice without exposing source or
/// ledger types.
///
/// The slice is wrapped in an in-memory source and validated under a default
/// budget, so a caller only needs `&[u8]`. Structural failures, budget exhaustion
/// and source failures stay typed in [`HsacoByteError`].
pub fn validate_hsaco_bytes(bytes: &[u8]) -> Result<HsacoSummary, HsacoByteError> {
    let source = MemorySource::from_bytes("hsaco-bytes", bytes.to_vec());
    let mut ledger = BudgetTracker::new(Budget::default());
    validate_hsaco(&source, &mut ledger).map_err(HsacoByteError::from)
}
