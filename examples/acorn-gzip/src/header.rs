//! Gzip member header parsing.

use acorn_core::ByteRange;

/// Gzip magic bytes `0x1F 0x8B`.
pub const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];

/// Deflate compression method in gzip.
pub const GZIP_CM_DEFLATE: u8 = 8;

const FLG_FEXTRA: u8 = 0x04;
const FLG_FNAME: u8 = 0x08;
const FLG_FCOMMENT: u8 = 0x10;
const FLG_FHCRC: u8 = 0x02;

/// Errors while parsing a gzip header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GzipHeaderError {
    /// Input is too short to contain a gzip header.
    Truncated,
    /// Magic bytes do not match gzip.
    NotGzip,
    /// Compression method is not supported.
    UnsupportedCompression(u8),
}

impl std::fmt::Display for GzipHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated gzip header"),
            Self::NotGzip => f.write_str("gzip magic mismatch"),
            Self::UnsupportedCompression(method) => write!(f, "unsupported gzip compression method {method}"),
        }
    }
}

impl std::error::Error for GzipHeaderError {}

/// Parsed gzip member header metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GzipHeader {
    /// Compression method byte from the header.
    pub compression_method: u8,
    /// Header flag byte.
    pub flags: u8,
    /// Total header byte length including optional fields.
    pub header_size: usize,
    /// Byte range of the deflate stream and gzip trailer inside the file.
    pub member_range: ByteRange,
}

/// Parses a single-member gzip header from `bytes`.
pub fn parse_gzip_header(bytes: &[u8]) -> Result<GzipHeader, GzipHeaderError> {
    if bytes.len() < 10 {
        return Err(GzipHeaderError::Truncated);
    }
    if bytes[0..2] != GZIP_MAGIC {
        return Err(GzipHeaderError::NotGzip);
    }
    let compression_method = bytes[2];
    if compression_method != GZIP_CM_DEFLATE {
        return Err(GzipHeaderError::UnsupportedCompression(compression_method));
    }
    let flags = bytes[3];
    let mut offset = 10usize;
    if flags & FLG_FEXTRA != 0 {
        if bytes.len() < offset + 2 {
            return Err(GzipHeaderError::Truncated);
        }
        let extra_len = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2 + extra_len;
    }
    if flags & FLG_FNAME != 0 {
        while offset < bytes.len() && bytes[offset] != 0 {
            offset += 1;
        }
        if offset >= bytes.len() {
            return Err(GzipHeaderError::Truncated);
        }
        offset += 1;
    }
    if flags & FLG_FCOMMENT != 0 {
        while offset < bytes.len() && bytes[offset] != 0 {
            offset += 1;
        }
        if offset >= bytes.len() {
            return Err(GzipHeaderError::Truncated);
        }
        offset += 1;
    }
    if flags & FLG_FHCRC != 0 {
        if bytes.len() < offset + 2 {
            return Err(GzipHeaderError::Truncated);
        }
        offset += 2;
    }
    if bytes.len() < offset + 8 {
        return Err(GzipHeaderError::Truncated);
    }
    let member_end = bytes.len();
    let member_range = ByteRange::new(offset as u64, (member_end - offset) as u64).map_err(|_| GzipHeaderError::Truncated)?;
    Ok(GzipHeader { compression_method, flags, header_size: offset, member_range })
}
