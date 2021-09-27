#![warn(missing_docs)]
//! ZIP archive indexing for Acorn document containers.

mod decode;
mod entry;
mod eocd;
mod index;
mod view;
mod zip64;

pub use decode::{MemberDecodeFacts, decode_member, decode_member_with_facts, read_member_payload};
pub use entry::ZipMember;
pub use eocd::{EndOfCentralDirectory, find_eocd};
pub use index::{ZipArchiveIndex, ZipIndexError, index_zip, index_zip_bytes};
pub use view::{
    COMPRESSION_DEFLATE, COMPRESSION_STORED, ZipViewError, member_payload_range, open_stored_member, read_raw_payload,
    read_stored_payload,
};
pub use zip64::{Zip64Fields, parse_zip64_extra};
