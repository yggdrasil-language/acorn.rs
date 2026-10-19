//! Zlib payload decode with parse-budget accounting.

use acorn_codec::{CodecError, CodecLimits, compression::Zlib};
use acorn_core::Budget;
use acorn_source::MemorySource;

use crate::header::{ZlibHeader, parse_zlib_header};

/// Decode evidence for one zlib stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZlibDecodeFacts {
    /// Compression method from CMF.
    pub compression_method: u8,
    /// Header byte length.
    pub header_size: usize,
    /// Decoded payload byte length.
    pub decoded_size: u64,
    /// Compressed member byte length including trailer.
    pub compressed_size: u64,
    /// Whether the zlib stream fully consumed the input.
    pub fully_consumed: bool,
    /// Bytes counted against the read budget.
    pub budget_read_bytes: u64,
    /// Bytes counted against the decode budget.
    pub budget_decoded_bytes: u64,
}

/// Errors while decoding zlib payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZlibDecodeError {
    /// Header parsing failed.
    Header(crate::header::ZlibHeaderError),
    /// Decode exceeded the parse budget.
    BudgetExceeded,
    /// Zlib decompression failed.
    DecompressFailed,
    /// Compressed input was not fully consumed.
    IncompleteInput,
}

impl std::fmt::Display for ZlibDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header(error) => write!(f, "zlib header error: {error}"),
            Self::BudgetExceeded => f.write_str("zlib decode budget exceeded"),
            Self::DecompressFailed => f.write_str("zlib decompression failed"),
            Self::IncompleteInput => f.write_str("zlib input was not fully consumed"),
        }
    }
}

impl std::error::Error for ZlibDecodeError {}

impl From<crate::header::ZlibHeaderError> for ZlibDecodeError {
    fn from(error: crate::header::ZlibHeaderError) -> Self {
        Self::Header(error)
    }
}

fn map_codec_error(error: CodecError) -> ZlibDecodeError {
    if error.is_resource_limit() { ZlibDecodeError::BudgetExceeded } else { ZlibDecodeError::DecompressFailed }
}

/// Decodes a single-member zlib stream and returns decode facts.
pub fn decode_zlib_with_facts(
    label: impl Into<String>,
    bytes: &[u8],
    budget: &Budget,
) -> Result<(MemorySource, ZlibDecodeFacts), ZlibDecodeError> {
    let header = parse_zlib_header(bytes)?;
    let budget_read_bytes = bytes.len() as u64;
    if budget_read_bytes > budget.max_read_bytes {
        return Err(ZlibDecodeError::BudgetExceeded);
    }
    let limits = CodecLimits::new().with_max_output_bytes(budget.max_decoded_bytes);
    let facts = Zlib::decode(bytes, limits).map_err(map_codec_error)?;
    if !facts.fully_consumed(bytes.len()) {
        return Err(ZlibDecodeError::IncompleteInput);
    }
    let budget_decoded_bytes = facts.value.len() as u64;
    if budget_decoded_bytes > budget.max_decoded_bytes {
        return Err(ZlibDecodeError::BudgetExceeded);
    }
    let decode_facts = ZlibDecodeFacts {
        compression_method: header.compression_method,
        header_size: header.header_size,
        decoded_size: budget_decoded_bytes,
        compressed_size: header.member_range.len,
        fully_consumed: facts.fully_consumed(bytes.len()),
        budget_read_bytes,
        budget_decoded_bytes,
    };
    Ok((MemorySource::from_bytes(label, facts.value), decode_facts))
}

/// Decodes a single-member zlib stream into a memory source.
pub fn decode_zlib(label: impl Into<String>, bytes: &[u8], budget: &Budget) -> Result<MemorySource, ZlibDecodeError> {
    let (decoded, _facts) = decode_zlib_with_facts(label, bytes, budget)?;
    Ok(decoded)
}

/// Returns the parsed zlib header for `bytes` without decoding the payload.
pub fn read_zlib_header(bytes: &[u8]) -> Result<ZlibHeader, ZlibDecodeError> {
    parse_zlib_header(bytes).map_err(ZlibDecodeError::from)
}
