//! Byte-slice validation entry that needs no caller-built source or ledger.

use std::fmt::{Display, Formatter};

use acorn_core::{Budget, BudgetError, BudgetTracker};
use acorn_source::{MemorySource, ReadError};

use crate::{DxbcLayoutError, DxbcSummary, validate_dxbc};

/// Failure from [`validate_dxbc_bytes`].
///
/// Each variant keeps the original typed error, so nothing is flattened into a
/// message string. Budget exhaustion is separated from structural failure even
/// though the layout pass reports it through [`DxbcLayoutError::Budget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxbcByteError {
    /// The container failed structural validation.
    Layout(DxbcLayoutError),
    /// The read allowance was exhausted.
    Budget(BudgetError),
    /// The backing source could not provide bytes.
    Source(ReadError),
}

impl Display for DxbcByteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout(error) => write!(f, "DXBC container could not be mapped: {error}"),
            Self::Budget(error) => write!(f, "DXBC read budget exhausted: {error}"),
            Self::Source(error) => write!(f, "DXBC byte source failed: {error}"),
        }
    }
}

impl std::error::Error for DxbcByteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Budget(error) => Some(error),
            Self::Source(error) => Some(error),
        }
    }
}

impl From<DxbcLayoutError> for DxbcByteError {
    fn from(error: DxbcLayoutError) -> Self {
        match error {
            DxbcLayoutError::Budget(error) => Self::Budget(error),
            other => Self::Layout(other),
        }
    }
}

impl From<BudgetError> for DxbcByteError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

impl From<ReadError> for DxbcByteError {
    fn from(error: ReadError) -> Self {
        Self::Source(error)
    }
}

/// Validates a complete DXBC byte slice without exposing source or ledger types.
///
/// The slice is wrapped in an in-memory source and validated under a default
/// budget, so a caller only needs `&[u8]`. Structural failures, budget exhaustion
/// and source failures stay typed in [`DxbcByteError`].
pub fn validate_dxbc_bytes(bytes: &[u8]) -> Result<DxbcSummary, DxbcByteError> {
    let source = MemorySource::from_bytes("dxbc-bytes", bytes.to_vec());
    let mut ledger = BudgetTracker::new(Budget::default());
    validate_dxbc(&source, &mut ledger).map_err(DxbcByteError::from)
}
