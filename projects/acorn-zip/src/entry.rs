use acorn_core::ByteRange;

use crate::time::ZipDosDateTime;

/// General-purpose bit flag: the member is encrypted.
pub const FLAG_ENCRYPTED: u16 = 1 << 0;
/// General-purpose bit flag: strong encryption is used (bit 6).
pub const FLAG_STRONG_ENCRYPTION: u16 = 1 << 6;
/// General-purpose bit flag: names and comments use UTF-8 (bit 11).
pub const FLAG_UTF8_NAME: u16 = 1 << 11;
/// General-purpose bit flag: sizes and CRC follow the payload in a data
/// descriptor (bit 3).
pub const FLAG_DATA_DESCRIPTOR: u16 = 1 << 3;
/// General-purpose bit flag: sizes are masked in the local header (bit 13).
pub const FLAG_MASKED_DATA: u16 = 1 << 13;

/// DOS attribute bit marking a directory.
pub const DOS_ATTRIBUTE_DIRECTORY: u8 = 0x10;

/// Unix file-type mask in the upper half of external attributes.
pub const UNIX_FILE_TYPE_MASK: u32 = 0xf000;
/// Unix directory file type.
pub const UNIX_DIRECTORY: u32 = 0x4000;

/// Host system recorded in `version made by` for Unix.
pub const HOST_UNIX: u8 = 3;
/// Host system recorded in `version made by` for macOS.
pub const HOST_MACOS: u8 = 19;

/// How a member name is encoded, as recorded by the general-purpose flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipNameEncoding {
    /// Bit 11 is set: the name bytes are UTF-8.
    Utf8,
    /// Bit 11 is clear: the name bytes are in the legacy encoding the writer chose.
    Unspecified,
}

/// Encryption classification derived only from general-purpose flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipEncryption {
    /// No encryption flag.
    None,
    /// Traditional PKWARE encryption (bit 0 only).
    Traditional,
    /// Strong encryption (bits 0 and 6).
    Strong,
}

/// One indexed ZIP member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipMember {
    /// Raw name bytes from the central directory.
    ///
    /// The bytes are not decoded here. [`ZipMember::name_encoding`] reports
    /// whether the writer declared UTF-8, and the caller chooses any fallback
    /// decoding order.
    pub raw_name: Vec<u8>,
    /// Version made by, including the host system in its upper byte.
    pub version_made_by: u16,
    /// Version needed to extract (central directory).
    pub version_needed: u16,
    /// General purpose bit flag (central directory).
    pub general_purpose_bit_flag: u16,
    /// Compression method from the central directory.
    pub compression_method: u16,
    /// CRC-32 from the central directory.
    pub crc32: u32,
    /// Raw DOS modification time and date from the central directory.
    pub modified: ZipDosDateTime,
    /// Modification time in Unix seconds when the extended timestamp extra exists.
    pub modified_unix: Option<i64>,
    /// Internal attributes from the central directory.
    pub internal_attributes: u16,
    /// External attributes from the central directory.
    ///
    /// The low byte holds DOS attributes and the upper half holds Unix mode bits
    /// when the writer was a Unix host.
    pub external_attributes: u32,
    /// Offset of the local file header.
    pub local_header_offset: u64,
    /// Compressed payload size.
    pub compressed_size: u64,
    /// Uncompressed payload size.
    pub uncompressed_size: u64,
    /// Byte range of the local header and payload in the archive file space.
    pub data_range: ByteRange,
    /// Byte range of the compressed payload only.
    pub payload_range: ByteRange,
}

impl ZipMember {
    /// How the member name is encoded.
    pub fn name_encoding(&self) -> ZipNameEncoding {
        if self.general_purpose_bit_flag & FLAG_UTF8_NAME != 0 { ZipNameEncoding::Utf8 } else { ZipNameEncoding::Unspecified }
    }

    /// Encryption kind declared by the general-purpose flags.
    pub fn encryption(&self) -> ZipEncryption {
        if self.general_purpose_bit_flag & FLAG_ENCRYPTED == 0 {
            return ZipEncryption::None;
        }
        if self.general_purpose_bit_flag & FLAG_STRONG_ENCRYPTION != 0 {
            ZipEncryption::Strong
        }
        else {
            ZipEncryption::Traditional
        }
    }

    /// Whether the member declares any encryption.
    pub fn is_encrypted(&self) -> bool {
        self.encryption() != ZipEncryption::None
    }

    /// Whether the local header masks sizes and CRC (bit 13).
    pub fn has_masked_data(&self) -> bool {
        self.general_purpose_bit_flag & FLAG_MASKED_DATA != 0
    }

    /// DOS attribute byte from the external attributes.
    pub fn dos_attributes(&self) -> u8 {
        (self.external_attributes & 0xff) as u8
    }

    /// Unix mode from the upper half of external attributes, when the writer was a
    /// Unix host.
    pub fn unix_mode(&self) -> Option<u32> {
        let host = (self.version_made_by >> 8) as u8;
        if host == HOST_UNIX || host == HOST_MACOS { Some(self.external_attributes >> 16) } else { None }
    }

    /// Whether the raw name ends with a forward slash.
    pub fn name_is_directory(&self) -> bool {
        self.raw_name.last() == Some(&b'/')
    }

    /// Whether the external attributes mark a directory.
    pub fn attributes_are_directory(&self) -> bool {
        if self.dos_attributes() & DOS_ATTRIBUTE_DIRECTORY != 0 {
            return true;
        }
        self.unix_mode().is_some_and(|mode| mode & UNIX_FILE_TYPE_MASK == UNIX_DIRECTORY)
    }

    /// Whether any ZIP directory marker is present.
    ///
    /// This reports format facts only. Path policy and whether a trailing slash is
    /// treated as a directory remain the caller's decision.
    pub fn is_directory(&self) -> bool {
        self.name_is_directory() || self.attributes_are_directory()
    }
}
