#![warn(missing_docs)]
//! Acorn domain diagnostic codes and builders on top of `diagnostic`.
//!
//! Stable `acorn.*` codes, budget usage, and conversion helpers. Rendering and
//! collection use the shared `diagnostic` crate.

pub use diagnostic::{
    Diagnostic, DiagnosticAction, DiagnosticCode as WireDiagnosticCode, DiagnosticLabel,
    DiagnosticLocation, DiagnosticOrigin, DiagnosticSeverity, DiagnosticSet, DiagnosticSink,
    Message, MessageArg,
};

/// Acorn domain diagnostic code (maps to dotted `acorn.*` wire identifiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticCode {
    MagicMismatch,
    Truncated,
    OffsetOverflow,
    OffsetOutOfRange,
    Alignment,
    ReferenceCycle,
    Overlap,
    Checksum,
    UnsupportedVersion,
    ResourceLimit,
    NeedRange,
}

impl DiagnosticCode {
    /// Stable dotted wire identifier for this code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MagicMismatch => "acorn.probe.magic-mismatch",
            Self::Truncated => "acorn.layout.truncated",
            Self::OffsetOverflow => "acorn.layout.offset-overflow",
            Self::OffsetOutOfRange => "acorn.layout.offset-out-of-range",
            Self::Alignment => "acorn.layout.alignment",
            Self::ReferenceCycle => "acorn.layout.reference-cycle",
            Self::Overlap => "acorn.layout.overlap",
            Self::Checksum => "acorn.layout.checksum",
            Self::UnsupportedVersion => "acorn.layout.unsupported-version",
            Self::ResourceLimit => "acorn.budget.resource-limit",
            Self::NeedRange => "acorn.layout.need-range",
        }
    }

    /// Producer component for [`DiagnosticOrigin`].
    pub const fn component(self) -> &'static str {
        match self {
            Self::MagicMismatch => "probe",
            Self::ResourceLimit => "budget",
            _ => "layout",
        }
    }

    /// Convert to the shared wire diagnostic code type.
    pub fn wire_code(self) -> WireDiagnosticCode {
        WireDiagnosticCode::new(self.as_str())
    }
}

/// Build an Acorn error diagnostic with fallback text.
pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
    build(code, DiagnosticSeverity::Error, message)
}

/// Build an Acorn warning diagnostic with fallback text.
pub fn warning(code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
    build(code, DiagnosticSeverity::Warning, message)
}

/// Build an Acorn info diagnostic with fallback text.
pub fn info(code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
    build(code, DiagnosticSeverity::Info, message)
}

fn build(code: DiagnosticCode, severity: DiagnosticSeverity, message: impl Into<String>) -> Diagnostic {
    let fallback = message.into();
    Diagnostic::new(
        code.wire_code(),
        severity,
        DiagnosticOrigin::new("acorn", code.component()),
        Message::new(code.as_str()).with_fallback(fallback),
    )
}

/// Structured recovery actions for Acorn diagnostics.
pub mod actions {
    use super::DiagnosticAction;

    /// Request additional byte ranges before continuing.
    pub fn provide_ranges() -> DiagnosticAction {
        DiagnosticAction::new("acorn.provide-ranges")
    }

    /// Reduce parse phase or projection depth.
    pub fn reduce_scope() -> DiagnosticAction {
        DiagnosticAction::new("acorn.reduce-scope")
    }

    /// Increase one or more parse budgets.
    pub fn increase_budget() -> DiagnosticAction {
        DiagnosticAction::new("acorn.increase-budget")
    }

    /// Try a different adapter or format candidate.
    pub fn try_alternate_adapter() -> DiagnosticAction {
        DiagnosticAction::new("acorn.try-alternate-adapter")
    }
}

/// Observable budget consumption attached to `ResourceLimited` results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BudgetUsage {
    pub read_bytes: u64,
    pub decoded_bytes: u64,
    pub nodes: u64,
    pub references: u64,
    pub pointer_hops: u64,
    pub scan_bytes: u64,
    pub cache_bytes: u64,
    pub envelope_depth: u32,
    pub container_members: u64,
}
