#![warn(missing_docs)]
//! Core contracts for Acorn binary layout parsing.
//!
//! This crate freezes address spaces, spans, partial semantics, node states,
//! and parse budgets. It does not perform I/O or format-specific decoding.

mod address;
mod budget;
pub mod diagnostic;
mod layout;
mod partial;
mod reference;
mod span;
mod state;
mod write_plan;

pub use address::{Address, AddressSpaceId, AddressSpaceKind, ByteRange, RangeError};
pub use budget::{BudgetError, BudgetTracker, ParseBudget};
pub use layout::{LayoutGraph, LayoutNode, LayoutNodeId, ReferenceEdge, evidence_absolute};
pub use partial::{Availability, NeedRange};
pub use reference::{JumpError, ReferenceEvidence, ReferenceKind, VisitKey, checked_absolute_jump, checked_relative_jump};
pub use span::{Provenance, ProvenanceStep, Span};
pub use state::NodeState;
pub use write_plan::{
    Endian, LayoutPlan, PatchPoint, PlanError, PlanSlot, PlanSlotId, Relocation, RelocationId, RelocationTarget,
};
