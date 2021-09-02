#![warn(missing_docs)]
//! ZIP archive indexing for Acorn document containers.

mod decode;
mod eocd;
mod entry;
mod index;
mod view;
mod zip64;

pub use decode::{decode_member, read_member_payload};
pub use eocd::{find_eocd, EndOfCentralDirectory};
pub use entry::ZipMember;
pub use index::{index_zip, index_zip_bytes, ZipArchiveIndex, ZipIndexError};
pub use view::{
    member_payload_range, open_stored_member, read_raw_payload, read_stored_payload, ZipViewError,
    COMPRESSION_DEFLATE, COMPRESSION_STORED,
};
