#![deny(missing_docs)]
//! Base data codecs for Acorn.
//!
//! This crate hosts stateless representation transforms and bounded stream
//! decoders. It does not depend on `acorn-core`, `acorn-source`, or format
//! adapters. Callers map [`CodecError`] and decode facts into parse budgets,
//! provenance, and format-specific diagnostics.
//!
//! # Transform families
//!
//! - [`bit`] — bit-level input and output primitives
//! - [`scalar`] — integer domain mappings (`ScalarTransform`)
//! - [`slice`] — one-shot slice transforms (`SliceCodec`)
//! - [`integer`] — integer byte representations (`ZigZag`, `Leb128`)
//! - [`representation`] — byte-to-text representations (`Base64`, `Hex`)
//! - [`compression`] — stream compression and its envelopes

mod error;
mod facts;
mod limits;
mod scalar;
mod slice;

pub use error::{ChecksumAlgorithm, CodecError, CodecFeature, EnvelopeKind, Ratio};
pub use facts::{ByteDecode, Canonicality, ChecksumFacts, DecodeFacts, StreamFacts};
pub use limits::CodecLimits;
pub use scalar::ScalarTransform;
pub use slice::SliceCodec;

/// Bit-level input and output primitives.
pub mod bit;
/// Integer domain transforms.
pub mod integer;
/// Byte representation transforms.
pub mod representation;

#[cfg(feature = "compression")]
/// Stream compression transforms.
pub mod compression;
