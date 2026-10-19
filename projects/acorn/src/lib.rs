#![deny(missing_docs)]
//! Public facade for the Acorn binary parsing stack.
//!
//! Most consumers should depend on `acorn` for the format-neutral Acorn
//! contracts and add a format adapter such as `acorn-zip` separately. This
//! facade does not register or re-export format adapters.

/// Acorn representation and compression codecs.
pub mod codec;
/// Acorn magic-byte probing and candidate contracts.
pub mod probe;
/// Acorn byte sources, windows, and sinks.
pub mod source;

pub use acorn_codec::{CodecError, CodecLimits};
pub use acorn_core::{
    Address, AddressSpaceId, AddressSpaceKind, Budget, ByteRange, LayoutGraph, LayoutNode, LayoutNodeId, NodeState, Provenance,
    Span,
};
pub use acorn_probe::{Confidence, ProbeAnchor, ProbeBudget, ProbeCandidate, ProbeSession, Signature};
pub use acorn_source::{ByteSink, ByteSource, ByteWindow, MemorySink, MemorySource, ReadOutcome, WriteError};
