#![warn(missing_docs)]
//! Magic-byte probing and format candidate collection for Acorn.
//!
//! Signatures produce ranked `ProbeCandidate` values with evidence ranges.
//! A magic hit alone is never treated as fully identified structure.

mod candidate;
mod probe;
mod signature;

pub use candidate::{Confidence, ProbeCandidate};
pub use probe::{
    document_container_signatures, probe_memory, ProbeBudget, ProbeSession,
};
pub use signature::{BytePattern, ProbeAnchor, Signature};
