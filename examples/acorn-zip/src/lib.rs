#![warn(missing_docs)]
//! ZIP archive indexing for Acorn document containers.

mod eocd;
mod entry;
mod index;
mod view;
mod zip64;

pub use eocd::EndOfCentralDirectory;
pub use entry::ZipMember;
pub use index::{index_zip, index_zip_bytes, ZipArchiveIndex, ZipIndexError};
pub use view::{
    member_payload_range, open_stored_member, read_stored_payload, ZipViewError,
    COMPRESSION_STORED,
};
