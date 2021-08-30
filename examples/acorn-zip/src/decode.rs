use std::io::Read;

use acorn_core::ParseBudget;
use acorn_source::{ByteSource, MemorySource, ReadOutcome};
use flate2::read::DeflateDecoder;

use crate::entry::ZipMember;
use crate::view::{read_raw_payload, ZipViewError, COMPRESSION_DEFLATE, COMPRESSION_STORED};

/// Materializes a member into a `Decoded` memory source (stored or deflate).
pub fn decode_member<S: ByteSource>(
    label: impl Into<String>,
    source: S,
    member: &ZipMember,
    budget: &ParseBudget,
) -> Result<MemorySource, ZipViewError> {
    let compressed = read_raw_payload(source, member)?;
    if compressed.len() as u64 > budget.max_read_bytes {
        return Err(ZipViewError::BudgetExceeded);
    }

    let decoded = match member.compression_method {
        COMPRESSION_STORED => {
            if member.uncompressed_size != member.compressed_size {
                return Err(ZipViewError::SizeMismatch);
            }
            compressed
        }
        COMPRESSION_DEFLATE => inflate_deflate(
            &compressed,
            member.uncompressed_size,
            budget.max_decoded_bytes,
        )?,
        other => return Err(ZipViewError::UnsupportedCompression(other)),
    };

    if decoded.len() as u64 > budget.max_decoded_bytes {
        return Err(ZipViewError::BudgetExceeded);
    }

    Ok(MemorySource::from_bytes(label, decoded))
}

fn inflate_deflate(
    input: &[u8],
    expected_uncompressed: u64,
    max_output: u64,
) -> Result<Vec<u8>, ZipViewError> {
    if expected_uncompressed > max_output {
        return Err(ZipViewError::BudgetExceeded);
    }
    let limit = if expected_uncompressed == 0 {
        max_output
    } else {
        expected_uncompressed.min(max_output)
    };
    let decoder = DeflateDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .take(limit)
        .read_to_end(&mut output)
        .map_err(|_| ZipViewError::DecompressFailed)?;
    if expected_uncompressed != 0 && output.len() as u64 != expected_uncompressed {
        return Err(ZipViewError::SizeMismatch);
    }
    Ok(output)
}

/// Reads and decodes a member payload into an owned buffer.
pub fn read_member_payload<S: ByteSource>(
    source: S,
    member: &ZipMember,
    budget: &ParseBudget,
) -> Result<Vec<u8>, ZipViewError> {
    let decoded = decode_member("zip-member", source, member, budget)?;
    let length = decoded.length().ok_or(ZipViewError::InvalidRange)? as usize;
    let mut buffer = vec![0u8; length];
    match decoded.read_at(0, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(ReadOutcome::Eof) => Err(ZipViewError::InvalidRange),
        Ok(ReadOutcome::Unavailable { .. }) => Err(ZipViewError::InvalidRange),
        Err(error) => Err(ZipViewError::Window(error.to_string())),
    }
}
