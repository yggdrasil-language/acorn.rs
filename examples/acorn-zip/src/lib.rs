#![warn(missing_docs)]
//! ZIP archive indexing for Acorn document containers.

mod eocd;
mod entry;
mod index;

pub use eocd::EndOfCentralDirectory;
pub use entry::ZipMember;
pub use index::{index_zip, index_zip_bytes, ZipArchiveIndex, ZipIndexError};
