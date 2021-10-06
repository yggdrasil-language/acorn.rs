#![warn(missing_docs)]
//! Zlib envelope indexing and decode for Acorn.

mod decode;
mod header;
mod index;

pub use decode::{ZlibDecodeError, ZlibDecodeFacts, decode_zlib, decode_zlib_with_facts, read_zlib_header};
pub use header::{ZLIB_CM_DEFLATE, ZlibHeader, ZlibHeaderError, parse_zlib_header};
pub use index::{ZlibArchiveIndex, ZlibIndexError, index_zlib, index_zlib_bytes};
