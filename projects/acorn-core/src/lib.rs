#![warn(missing_docs)]
//! Core contracts for Acorn binary layout parsing.
//!
//! This crate freezes address spaces, spans, partial semantics, node states,
//! and parse budgets. It does not perform I/O or format-specific decoding.

mod address;
mod budget;
mod layout;
mod partial;
mod reference;
mod span;
mod state;

pub use address::{Address, AddressSpaceId, AddressSpaceKind, ByteRange, RangeError};
pub use budget::{BudgetError, BudgetTracker, ParseBudget};
pub use layout::{evidence_absolute, LayoutGraph, LayoutNode, LayoutNodeId, ReferenceEdge};
pub use partial::{Availability, NeedRange};
pub use reference::{
    checked_absolute_jump, checked_relative_jump, JumpError, ReferenceEvidence, ReferenceKind,
    VisitKey,
};
pub use span::{Provenance, ProvenanceStep, Span};
pub use state::NodeState;
