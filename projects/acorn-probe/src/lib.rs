#![deny(missing_docs)]
//! Magic-byte probing and format candidate collection for Acorn.
//!
//! Signatures produce ranked `ProbeCandidate` values with evidence ranges.
//! A magic hit alone is never treated as fully identified structure.

mod candidate;
mod probe;
mod signature;

pub use candidate::{Confidence, ProbeCandidate};
pub use probe::{ProbeBudget, ProbeSession, document_container_signatures, probe_memory};
pub use signature::{BytePattern, ProbeAnchor, Signature};
