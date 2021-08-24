#![warn(missing_docs)]
//! Core contracts for Acorn binary layout parsing.
//!
//! This crate freezes address spaces, spans, partial semantics, node states,
//! and parse budgets. It does not perform I/O or format-specific decoding.

mod address;
mod budget;
mod partial;
mod span;
mod state;

pub use address::{Address, AddressSpaceId, AddressSpaceKind, ByteRange, RangeError};
pub use budget::{BudgetError, BudgetTracker, ParseBudget};
pub use partial::{Availability, NeedRange};
pub use span::{Provenance, ProvenanceStep, Span};
pub use state::NodeState;
