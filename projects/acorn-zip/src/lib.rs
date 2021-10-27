#![warn(missing_docs)]
//! ZIP archive indexing for Acorn document containers.

mod crc;
mod decode;
mod entry;
mod eocd;
mod index;
mod time;
mod view;
mod writer;
mod zip64;

pub use crc::ieee_crc32;
pub use decode::{MemberDecodeFacts, decode_member, decode_member_with_facts, read_member_payload};
pub use entry::{
    DOS_ATTRIBUTE_DIRECTORY, FLAG_DATA_DESCRIPTOR, FLAG_ENCRYPTED, FLAG_MASKED_DATA, FLAG_STRONG_ENCRYPTION, FLAG_UTF8_NAME,
    ZipEncryption, ZipMember, ZipNameEncoding,
};
pub use eocd::{EndOfCentralDirectory, find_eocd, scan_eocd_fields};
pub use index::{ZipArchiveIndex, ZipIndexError, index_zip, index_zip_bytes};
pub use time::{
    DOS_MAX_YEAR, DOS_MIN_YEAR, EXTENDED_TIMESTAMP_EXTRA_ID, ZipDateTime, ZipDosDateTime, ZipExtendedTimestamp,
    parse_extended_timestamp,
};
pub use view::{
    COMPRESSION_DEFLATE, COMPRESSION_STORED, ZipViewError, member_payload_range, open_stored_member, read_raw_payload,
    read_stored_payload,
};
pub use writer::{Zip64Mode, ZipCompression, ZipEntry, ZipStreamEntry, ZipStreamWriter, ZipWriteError, ZipWriter};
pub use zip64::{Zip64Fields, parse_zip64_extra};
