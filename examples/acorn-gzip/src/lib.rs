#![warn(missing_docs)]
//! Gzip envelope indexing and decode for Acorn.

mod decode;
mod header;
mod index;

pub use decode::{GzipDecodeError, GzipDecodeFacts, decode_gzip, decode_gzip_with_facts, read_gzip_header};
pub use header::{GZIP_CM_DEFLATE, GZIP_MAGIC, GzipHeader, GzipHeaderError, parse_gzip_header};
pub use index::{GzipArchiveIndex, GzipIndexError, index_gzip, index_gzip_bytes};
