#![warn(missing_docs)]
//! Legacy Word `.doc` binary semantics for Acorn.

mod document;
mod error;
mod text;

pub use document::{ExtractedText, LegacyDoc, extract_text_from_bytes, open_legacy_doc};
pub use error::DocError;
pub use text::{decode_piece_table, decode_text_range, fib_clx_location};
