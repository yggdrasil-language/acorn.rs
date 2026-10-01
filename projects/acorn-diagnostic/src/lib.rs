#![warn(missing_docs)]
//! Structured diagnostics for the Acorn binary parser.
//!
//! This crate owns stable diagnostic codes, severity, and the machine-facing
//! diagnostic record. Rendering to text or other frontends is a separate concern.

use std::collections::BTreeMap;

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    /// An error that blocks further validation or projection.
    Error,
    /// A warning that should be surfaced but does not invalidate structure.
    Warning,
    /// Informational note or recoverable condition.
    Info,
}

/// Stable diagnostic codes shared across all format adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticCode {
    /// Magic bytes or signature did not match the expected pattern.
    MagicMismatch,
    /// Declared content extends past the available source length.
    Truncated,
    /// Offset arithmetic overflowed.
    OffsetOverflow,
    /// Offset points outside the address space bounds.
    OffsetOutOfRange,
    /// Field or record violates alignment constraints.
    Alignment,
    /// Reference traversal detected a cycle.
    ReferenceCycle,
    /// Two layout claims overlap in the same address space.
    Overlap,
    /// Checksum or hash verification failed.
    Checksum,
    /// Structure is recognizable but the adapter does not support this variant.
    UnsupportedVersion,
    /// Parsing stopped because a budget limit was reached.
    ResourceLimit,
    /// Additional source bytes are required before continuing.
    NeedRange,
}

impl DiagnosticCode {
    /// Returns the stable string identifier for this code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MagicMismatch => "ACORN_MAGIC_MISMATCH",
            Self::Truncated => "ACORN_TRUNCATED",
            Self::OffsetOverflow => "ACORN_OFFSET_OVERFLOW",
            Self::OffsetOutOfRange => "ACORN_OFFSET_OUT_OF_RANGE",
            Self::Alignment => "ACORN_ALIGNMENT",
            Self::ReferenceCycle => "ACORN_REFERENCE_CYCLE",
            Self::Overlap => "ACORN_OVERLAP",
            Self::Checksum => "ACORN_CHECKSUM",
            Self::UnsupportedVersion => "ACORN_UNSUPPORTED_VERSION",
            Self::ResourceLimit => "ACORN_RESOURCE_LIMIT",
            Self::NeedRange => "ACORN_NEED_RANGE",
        }
    }
}

/// A recoverable action suggested alongside a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RecoveryAction {
    /// Provide the listed byte ranges and retry the same query.
    ProvideRanges,
    /// Reduce the requested parse phase or projection depth.
    ReduceScope,
    /// Increase one or more parse budgets.
    IncreaseBudget,
    /// Switch to a different adapter or format candidate.
    TryAlternateAdapter,
}

/// A structured diagnostic record.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    /// Stable code.
    pub code: DiagnosticCode,
    /// Severity.
    pub severity: Severity,
    /// Human-readable summary. Frontends may localize this string.
    pub message: String,
    /// Structured parameters for localization or machine inspection.
    pub params: BTreeMap<String, String>,
    /// Suggested recovery actions, if any.
    pub recovery: Vec<RecoveryAction>,
}

impl Diagnostic {
    /// Creates a new error diagnostic.
    pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            params: BTreeMap::new(),
            recovery: Vec::new(),
        }
    }

    /// Creates a new warning diagnostic.
    pub fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            params: BTreeMap::new(),
            recovery: Vec::new(),
        }
    }

    /// Attaches a structured parameter.
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// Attaches a recovery action.
    pub fn with_recovery(mut self, action: RecoveryAction) -> Self {
        self.recovery.push(action);
        self
    }
}

/// Observable budget consumption attached to `ResourceLimited` results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BudgetUsage {
    /// Bytes read from raw sources.
    pub read_bytes: u64,
    /// Bytes materialized in decoded address spaces.
    pub decoded_bytes: u64,
    /// Layout nodes allocated.
    pub nodes: u64,
    /// Reference edges followed or recorded.
    pub references: u64,
    /// Pointer hops performed during traversal.
    pub pointer_hops: u64,
    /// Bytes scanned during signature or window search.
    pub scan_bytes: u64,
    /// Bytes retained in caches.
    pub cache_bytes: u64,
    /// Envelope nesting depth reached.
    pub envelope_depth: u32,
    /// Container members indexed.
    pub container_members: u64,
}
