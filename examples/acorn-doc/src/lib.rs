#![warn(missing_docs)]
//! Legacy Word `.doc` binary semantics for Acorn.

mod document;
mod error;
mod text;

pub use document::{extract_text_from_bytes, open_legacy_doc, ExtractedText, LegacyDoc};
pub use error::DocError;
