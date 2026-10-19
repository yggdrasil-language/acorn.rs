//! Gzip payload decode with parse-budget accounting.

use acorn_codec::{CodecError, CodecLimits, compression::Gzip};
use acorn_core::Budget;
use acorn_source::MemorySource;

use crate::header::{GzipHeader, parse_gzip_header};

/// Decode evidence for one gzip member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GzipDecodeFacts {
    /// Compression method from the header.
    pub compression_method: u8,
    /// Header byte length.
    pub header_size: usize,
    /// Decoded payload byte length.
    pub decoded_size: u64,
    /// Compressed member byte length including trailer.
    pub compressed_size: u64,
    /// Whether the gzip stream fully consumed the input.
    pub fully_consumed: bool,
    /// Bytes counted against the read budget.
    pub budget_read_bytes: u64,
    /// Bytes counted against the decode budget.
    pub budget_decoded_bytes: u64,
}

/// Errors while decoding gzip payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GzipDecodeError {
    /// Header parsing failed.
    Header(crate::header::GzipHeaderError),
    /// Decode exceeded the parse budget.
    BudgetExceeded,
    /// Gzip decompression failed.
    DecompressFailed,
    /// Compressed input was not fully consumed.
    IncompleteInput,
}

impl std::fmt::Display for GzipDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header(error) => write!(f, "gzip header error: {error}"),
            Self::BudgetExceeded => f.write_str("gzip decode budget exceeded"),
            Self::DecompressFailed => f.write_str("gzip decompression failed"),
            Self::IncompleteInput => f.write_str("gzip input was not fully consumed"),
        }
    }
}

impl std::error::Error for GzipDecodeError {}

impl From<crate::header::GzipHeaderError> for GzipDecodeError {
    fn from(error: crate::header::GzipHeaderError) -> Self {
        Self::Header(error)
    }
}

fn map_codec_error(error: CodecError) -> GzipDecodeError {
    if error.is_resource_limit() { GzipDecodeError::BudgetExceeded } else { GzipDecodeError::DecompressFailed }
}

/// Decodes a single-member gzip file and returns decode facts.
pub fn decode_gzip_with_facts(
    label: impl Into<String>,
    bytes: &[u8],
    budget: &Budget,
) -> Result<(MemorySource, GzipDecodeFacts), GzipDecodeError> {
    let header = parse_gzip_header(bytes)?;
    let budget_read_bytes = bytes.len() as u64;
    if budget_read_bytes > budget.max_read_bytes {
        return Err(GzipDecodeError::BudgetExceeded);
    }
    let limits = CodecLimits::new().with_max_output_bytes(budget.max_decoded_bytes);
    let facts = Gzip::decode(bytes, limits).map_err(map_codec_error)?;
    if !facts.fully_consumed(bytes.len()) {
        return Err(GzipDecodeError::IncompleteInput);
    }
    let budget_decoded_bytes = facts.value.len() as u64;
    if budget_decoded_bytes > budget.max_decoded_bytes {
        return Err(GzipDecodeError::BudgetExceeded);
    }
    let decode_facts = GzipDecodeFacts {
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

/// Decodes a single-member gzip file into a memory source.
pub fn decode_gzip(label: impl Into<String>, bytes: &[u8], budget: &Budget) -> Result<MemorySource, GzipDecodeError> {
    let (decoded, _facts) = decode_gzip_with_facts(label, bytes, budget)?;
    Ok(decoded)
}

/// Returns the parsed gzip header for `bytes` without decoding the payload.
pub fn read_gzip_header(bytes: &[u8]) -> Result<GzipHeader, GzipDecodeError> {
    parse_gzip_header(bytes).map_err(GzipDecodeError::from)
}
