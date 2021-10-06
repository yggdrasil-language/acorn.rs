//! Zlib wrapper header parsing.

use acorn_core::ByteRange;

/// Deflate compression method in zlib.
pub const ZLIB_CM_DEFLATE: u8 = 8;

/// Errors while parsing a zlib header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZlibHeaderError {
    /// Input is too short to contain a zlib header.
    Truncated,
    /// Header checksum does not match zlib rules.
    NotZlib,
    /// Compression method is not supported.
    UnsupportedCompression(u8),
}

impl std::fmt::Display for ZlibHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated zlib header"),
            Self::NotZlib => f.write_str("zlib header checksum mismatch"),
            Self::UnsupportedCompression(method) => write!(f, "unsupported zlib compression method {method}"),
        }
    }
}

impl std::error::Error for ZlibHeaderError {}

/// Parsed zlib wrapper metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZlibHeader {
    /// Compression method byte from CMF.
    pub compression_method: u8,
    /// CMF byte.
    pub cmf: u8,
    /// FLG byte.
    pub flg: u8,
    /// Total header byte length.
    pub header_size: usize,
    /// Byte range of the deflate stream and Adler-32 trailer.
    pub member_range: ByteRange,
}

/// Parses a single-member zlib wrapper from `bytes`.
pub fn parse_zlib_header(bytes: &[u8]) -> Result<ZlibHeader, ZlibHeaderError> {
    if bytes.len() < 6 {
        return Err(ZlibHeaderError::Truncated);
    }
    let cmf = bytes[0];
    let flg = bytes[1];
    let compression_method = cmf & 0x0F;
    if compression_method != ZLIB_CM_DEFLATE {
        return Err(ZlibHeaderError::UnsupportedCompression(compression_method));
    }
    let header_check = (u16::from(cmf) << 8) | u16::from(flg);
    if header_check % 31 != 0 {
        return Err(ZlibHeaderError::NotZlib);
    }
    let header_size = 2usize;
    let member_range =
        ByteRange::new(header_size as u64, (bytes.len() - header_size) as u64).map_err(|_| ZlibHeaderError::Truncated)?;
    Ok(ZlibHeader { compression_method, cmf, flg, header_size, member_range })
}
