//! ZIP writer for stored and Deflate members.
//!
//! The writer emits APPNOTE records directly: local headers, the central
//! directory, ZIP64 records when needed and the end-of-central-directory record.
//! Deflate bytes come from `acorn-codec`; no external archive or compression
//! backend is involved.

use acorn_codec::compression::{Deflate, deflate::DeflateLevel};

use crate::{
    crc::ieee_crc32,
    entry::{DOS_ATTRIBUTE_DIRECTORY, FLAG_UTF8_NAME},
    time::{EXTENDED_TIMESTAMP_EXTRA_ID, ZipDosDateTime},
    zip64::ZIP64_EXTRA_ID,
};

const LOCAL_HEADER_SIG: u32 = 0x0403_4b50;
const CENTRAL_HEADER_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
const ZIP64_EOCD_SIG: u32 = 0x0606_4b50;
const ZIP64_EOCD_LOCATOR_SIG: u32 = 0x0706_4b50;

/// Compression method for a written member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZipCompression {
    /// Store the payload without compression.
    #[default]
    Store,
    /// Compress the payload with `acorn-codec`'s raw Deflate encoder.
    Deflate,
}

impl ZipCompression {
    fn method(self) -> u16 {
        match self {
            Self::Store => 0,
            Self::Deflate => 8,
        }
    }

    fn encode(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Store => data.to_vec(),
            Self::Deflate => Deflate::encode(data, DeflateLevel::Default),
        }
    }
}

/// ZIP64 emission policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Zip64Mode {
    /// Emit ZIP64 records only when a value does not fit its classic field.
    #[default]
    Auto,
    /// Emit ZIP64 records for every member.
    ///
    /// Used by tests and by callers that want the 64-bit layout regardless of
    /// current sizes.
    Always,
}

/// One member to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Raw member name bytes.
    pub name: Vec<u8>,
    /// Uncompressed payload.
    pub data: Vec<u8>,
    /// Compression method.
    pub compression: ZipCompression,
    /// DOS modification time written to both headers.
    pub modified: ZipDosDateTime,
    /// Optional extended-timestamp modification time in Unix seconds.
    pub modified_unix: Option<i64>,
    /// Version made by, including the host system in its upper byte.
    pub version_made_by: u16,
    /// Internal attributes.
    pub internal_attributes: u16,
    /// External attributes.
    pub external_attributes: u32,
    /// Whether the general-purpose UTF-8 name flag should be set.
    pub name_is_utf8: bool,
}

impl ZipEntry {
    /// Creates a stored member with default header attributes.
    pub fn stored(name: impl AsRef<[u8]>, data: impl Into<Vec<u8>>) -> Self {
        Self::new(name, data, ZipCompression::Store)
    }

    /// Creates a Deflate member with default header attributes.
    pub fn deflated(name: impl AsRef<[u8]>, data: impl Into<Vec<u8>>) -> Self {
        Self::new(name, data, ZipCompression::Deflate)
    }

    /// Creates a `folder/` directory member with a DOS directory attribute.
    pub fn directory(name: impl AsRef<[u8]>, modified: ZipDosDateTime) -> Self {
        let mut entry = Self::stored(name, Vec::new());
        entry.modified = modified;
        entry.external_attributes = u32::from(DOS_ATTRIBUTE_DIRECTORY);
        entry
    }

    fn new(name: impl AsRef<[u8]>, data: impl Into<Vec<u8>>, compression: ZipCompression) -> Self {
        Self {
            name: name.as_ref().to_vec(),
            data: data.into(),
            compression,
            modified: ZipDosDateTime::ZERO,
            modified_unix: None,
            version_made_by: 20,
            internal_attributes: 0,
            external_attributes: 0,
            name_is_utf8: true,
        }
    }
}

/// Fatal problems while assembling an archive in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipWriteError {
    /// A member name does not fit the 16-bit name length field.
    NameTooLong {
        /// Byte length of the name.
        length: usize,
    },
    /// The UTF-8 flag was requested for bytes that are not valid UTF-8.
    InvalidUtf8Name,
    /// The combined extra fields do not fit the 16-bit extra length field.
    ExtraFieldTooLong {
        /// Byte length of the extra block.
        length: usize,
    },
    /// An extended-timestamp value does not fit the 32-bit extra field.
    ExtendedTimestampOutOfRange {
        /// Supplied Unix timestamp.
        value: i64,
    },
}

impl std::fmt::Display for ZipWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NameTooLong { length } => write!(f, "member name is too long: {length} bytes"),
            Self::InvalidUtf8Name => write!(f, "member name is not valid UTF-8 but the UTF-8 flag is set"),
            Self::ExtraFieldTooLong { length } => write!(f, "member extra fields are too long: {length} bytes"),
            Self::ExtendedTimestampOutOfRange { value } => {
                write!(f, "extended timestamp does not fit 32 bits: {value}")
            }
        }
    }
}

impl std::error::Error for ZipWriteError {}

/// Builds a ZIP archive in memory.
#[derive(Debug, Clone, Default)]
pub struct ZipWriter {
    entries: Vec<ZipEntry>,
    zip64: Zip64Mode,
}

impl ZipWriter {
    /// Creates an empty writer with the automatic ZIP64 policy.
    pub const fn new() -> Self {
        Self { entries: Vec::new(), zip64: Zip64Mode::Auto }
    }

    /// Creates an empty writer with an explicit ZIP64 policy.
    pub const fn with_zip64(zip64: Zip64Mode) -> Self {
        Self { entries: Vec::new(), zip64 }
    }

    /// Adds one member.
    pub fn push(&mut self, entry: ZipEntry) {
        self.entries.push(entry);
    }

    /// Returns the members queued for writing.
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// Writes every queued member and returns the complete archive bytes.
    pub fn into_bytes(self) -> Result<Vec<u8>, ZipWriteError> {
        write_archive(self.entries, self.zip64)
    }
}

struct PreparedEntry {
    name: Vec<u8>,
    offset: u64,
    method: u16,
    flags: u16,
    crc: u32,
    uncompressed_size: u64,
    compressed_size: u64,
    modified: ZipDosDateTime,
    modified_unix: Option<i64>,
    version_made_by: u16,
    internal_attributes: u16,
    external_attributes: u32,
    zip64: bool,
}

fn write_archive(entries: Vec<ZipEntry>, zip64: Zip64Mode) -> Result<Vec<u8>, ZipWriteError> {
    let mut output = Vec::new();
    let mut prepared = Vec::with_capacity(entries.len());

    for entry in entries {
        if entry.name.len() > u16::MAX as usize {
            return Err(ZipWriteError::NameTooLong { length: entry.name.len() });
        }
        if entry.name_is_utf8 && std::str::from_utf8(&entry.name).is_err() {
            return Err(ZipWriteError::InvalidUtf8Name);
        }
        let timestamp_extra = entry.modified_unix.map(extended_timestamp_extra).transpose()?;
        let compressed = entry.compression.encode(&entry.data);
        let uncompressed_size = entry.data.len() as u64;
        let compressed_size = compressed.len() as u64;
        let crc = ieee_crc32(&entry.data);
        let offset = output.len() as u64;
        let use_zip64 = match zip64 {
            Zip64Mode::Always => true,
            Zip64Mode::Auto => {
                uncompressed_size >= u32::MAX as u64 || compressed_size >= u32::MAX as u64 || offset >= u32::MAX as u64
            }
        };
        let flags = if entry.name_is_utf8 { FLAG_UTF8_NAME } else { 0 };
        let method = entry.compression.method();

        let mut local_extra = Vec::new();
        if use_zip64 {
            push_zip64_extra(&mut local_extra, false, uncompressed_size, compressed_size, offset);
        }
        if let Some(extra) = &timestamp_extra {
            local_extra.extend_from_slice(extra);
        }
        check_extra_length(local_extra.len())?;

        write_local_header(
            &mut output,
            &entry.name,
            &local_extra,
            if use_zip64 { 45 } else { 20 },
            flags,
            method,
            entry.modified,
            crc,
            uncompressed_size,
            compressed_size,
            use_zip64,
        );
        output.extend_from_slice(&compressed);

        prepared.push(PreparedEntry {
            name: entry.name,
            offset,
            method,
            flags,
            crc,
            uncompressed_size,
            compressed_size,
            modified: entry.modified,
            modified_unix: entry.modified_unix,
            version_made_by: entry.version_made_by,
            internal_attributes: entry.internal_attributes,
            external_attributes: entry.external_attributes,
            zip64: use_zip64,
        });
    }

    let central_directory_offset = output.len() as u64;
    for entry in &prepared {
        write_central_header(&mut output, entry)?;
    }
    let central_directory_size = output.len() as u64 - central_directory_offset;

    let entry_count = prepared.len() as u64;
    let needs_zip64 = match zip64 {
        Zip64Mode::Always => true,
        Zip64Mode::Auto => {
            entry_count >= u16::MAX as u64
                || central_directory_size >= u32::MAX as u64
                || central_directory_offset >= u32::MAX as u64
        }
    };

    if needs_zip64 {
        let zip64_offset = output.len() as u64;
        write_zip64_eocd(&mut output, entry_count, central_directory_size, central_directory_offset);
        write_zip64_locator(&mut output, zip64_offset);
    }

    write_eocd(&mut output, entry_count, central_directory_size, central_directory_offset, needs_zip64);
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn write_local_header(
    output: &mut Vec<u8>,
    name: &[u8],
    extra: &[u8],
    version_needed: u16,
    flags: u16,
    method: u16,
    modified: ZipDosDateTime,
    crc: u32,
    uncompressed_size: u64,
    compressed_size: u64,
    zip64: bool,
) {
    output.extend_from_slice(&LOCAL_HEADER_SIG.to_le_bytes());
    put_u16(output, version_needed);
    put_u16(output, flags);
    put_u16(output, method);
    put_u16(output, modified.time);
    put_u16(output, modified.date);
    put_u32(output, crc);
    put_u32(output, classic_size(compressed_size, zip64));
    put_u32(output, classic_size(uncompressed_size, zip64));
    put_u16(output, name.len() as u16);
    put_u16(output, extra.len() as u16);
    output.extend_from_slice(name);
    output.extend_from_slice(extra);
}

fn write_central_header(output: &mut Vec<u8>, entry: &PreparedEntry) -> Result<(), ZipWriteError> {
    let mut extra = Vec::new();
    if entry.zip64 {
        push_zip64_extra(&mut extra, true, entry.uncompressed_size, entry.compressed_size, entry.offset);
    }
    if let Some(timestamp) = entry.modified_unix {
        extra.extend_from_slice(&extended_timestamp_extra(timestamp)?);
    }
    check_extra_length(extra.len())?;

    output.extend_from_slice(&CENTRAL_HEADER_SIG.to_le_bytes());
    put_u16(output, entry.version_made_by);
    put_u16(output, if entry.zip64 { 45 } else { 20 });
    put_u16(output, entry.flags);
    put_u16(output, entry.method);
    put_u16(output, entry.modified.time);
    put_u16(output, entry.modified.date);
    put_u32(output, entry.crc);
    put_u32(output, classic_size(entry.compressed_size, entry.zip64));
    put_u32(output, classic_size(entry.uncompressed_size, entry.zip64));
    put_u16(output, entry.name.len() as u16);
    put_u16(output, extra.len() as u16);
    put_u16(output, 0);
    put_u16(output, 0);
    put_u16(output, entry.internal_attributes);
    put_u32(output, entry.external_attributes);
    put_u32(output, classic_size(entry.offset, entry.zip64));
    output.extend_from_slice(&entry.name);
    output.extend_from_slice(&extra);
    Ok(())
}

fn write_eocd(output: &mut Vec<u8>, entry_count: u64, central_directory_size: u64, central_directory_offset: u64, zip64: bool) {
    let entry_count = if zip64 { u16::MAX } else { entry_count as u16 };
    let size = if zip64 { u32::MAX } else { central_directory_size as u32 };
    let offset = if zip64 { u32::MAX } else { central_directory_offset as u32 };
    output.extend_from_slice(&EOCD_SIG.to_le_bytes());
    put_u16(output, 0);
    put_u16(output, 0);
    put_u16(output, entry_count);
    put_u16(output, entry_count);
    put_u32(output, size);
    put_u32(output, offset);
    put_u16(output, 0);
}

fn write_zip64_eocd(output: &mut Vec<u8>, entry_count: u64, central_directory_size: u64, central_directory_offset: u64) {
    output.extend_from_slice(&ZIP64_EOCD_SIG.to_le_bytes());
    put_u64(output, 44);
    put_u16(output, 45);
    put_u16(output, 45);
    put_u32(output, 0);
    put_u32(output, 0);
    put_u64(output, entry_count);
    put_u64(output, entry_count);
    put_u64(output, central_directory_size);
    put_u64(output, central_directory_offset);
}

fn write_zip64_locator(output: &mut Vec<u8>, zip64_eocd_offset: u64) {
    output.extend_from_slice(&ZIP64_EOCD_LOCATOR_SIG.to_le_bytes());
    put_u32(output, 0);
    put_u64(output, zip64_eocd_offset);
    put_u32(output, 1);
}

fn classic_size(value: u64, zip64: bool) -> u32 {
    if zip64 { u32::MAX } else { value as u32 }
}

fn push_zip64_extra(extra: &mut Vec<u8>, include_offset: bool, uncompressed_size: u64, compressed_size: u64, offset: u64) {
    let data_size = if include_offset { 24 } else { 16 };
    put_u16(extra, ZIP64_EXTRA_ID);
    put_u16(extra, data_size);
    put_u64(extra, uncompressed_size);
    put_u64(extra, compressed_size);
    if include_offset {
        put_u64(extra, offset);
    }
}

fn extended_timestamp_extra(value: i64) -> Result<Vec<u8>, ZipWriteError> {
    let value = i32::try_from(value).map_err(|_| ZipWriteError::ExtendedTimestampOutOfRange { value })?;
    let mut extra = Vec::with_capacity(9);
    put_u16(&mut extra, EXTENDED_TIMESTAMP_EXTRA_ID);
    put_u16(&mut extra, 5);
    extra.push(0x01);
    put_u32(&mut extra, value as u32);
    Ok(extra)
}

fn check_extra_length(length: usize) -> Result<(), ZipWriteError> {
    if length > u16::MAX as usize {
        return Err(ZipWriteError::ExtraFieldTooLong { length });
    }
    Ok(())
}

fn put_u16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}
