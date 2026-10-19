use acorn_codec::{CodecError, CodecLimits, compression::RawDeflate};
use acorn_core::Budget;
use acorn_source::{ByteSource, MemorySource, ReadOutcome};

use crate::{
    entry::ZipMember,
    view::{COMPRESSION_DEFLATE, COMPRESSION_STORED, ZipViewError, read_raw_payload},
};

/// Independent decode evidence for one ZIP member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDecodeFacts {
    /// Compression method that was decoded.
    pub method: u16,
    /// Decoded payload byte length.
    pub decoded_size: u64,
    /// Compressed payload byte length that was read.
    pub compressed_size: u64,
    /// Whether the compressed input was fully consumed.
    pub fully_consumed: bool,
    /// CRC32 from the central directory.
    pub crc32_expected: u32,
    /// CRC32 of the decoded bytes.
    pub crc32_actual: u32,
    /// Bytes counted against the read budget.
    pub budget_read_bytes: u64,
    /// Bytes counted against the decode budget.
    pub budget_decoded_bytes: u64,
}

impl MemberDecodeFacts {
    /// Whether CRC and consumption evidence pass.
    pub fn checksum_ok(&self) -> bool {
        self.fully_consumed && self.crc32_actual == self.crc32_expected
    }
}

/// Materializes a member and returns decode facts (CRC / consumption).
pub fn decode_member_with_facts<S: ByteSource>(
    label: impl Into<String>,
    source: S,
    member: &ZipMember,
    budget: &Budget,
) -> Result<(MemorySource, MemberDecodeFacts), ZipViewError> {
    let compressed = read_raw_payload(source, member)?;
    let budget_read_bytes = compressed.len() as u64;
    if budget_read_bytes > budget.max_read_bytes {
        return Err(ZipViewError::BudgetExceeded);
    }

    let (decoded, fully_consumed) = match member.compression_method {
        COMPRESSION_STORED => {
            if member.uncompressed_size != member.compressed_size {
                return Err(ZipViewError::SizeMismatch);
            }
            if compressed.len() as u64 != member.compressed_size {
                return Err(ZipViewError::SizeMismatch);
            }
            (compressed, true)
        }
        COMPRESSION_DEFLATE => {
            if member.uncompressed_size > budget.max_decoded_bytes {
                return Err(ZipViewError::BudgetExceeded);
            }
            let limits = CodecLimits::new().with_max_output_bytes(budget.max_decoded_bytes);
            let facts = RawDeflate::decode(&compressed, limits).map_err(map_codec_error)?;
            let fully_consumed = facts.deflate_stream_complete(compressed.len());
            (facts.value, fully_consumed)
        }
        other => return Err(ZipViewError::UnsupportedCompression(other)),
    };

    let budget_decoded_bytes = decoded.len() as u64;
    if budget_decoded_bytes > budget.max_decoded_bytes {
        return Err(ZipViewError::BudgetExceeded);
    }
    if member.uncompressed_size != 0 && budget_decoded_bytes != member.uncompressed_size {
        return Err(ZipViewError::SizeMismatch);
    }

    let crc32_actual = ieee_crc32(&decoded);
    let facts = MemberDecodeFacts {
        method: member.compression_method,
        decoded_size: budget_decoded_bytes,
        compressed_size: budget_read_bytes,
        fully_consumed,
        crc32_expected: member.crc32,
        crc32_actual,
        budget_read_bytes,
        budget_decoded_bytes,
    };
    if !fully_consumed {
        return Err(ZipViewError::IncompleteCompressedInput);
    }
    if crc32_actual != member.crc32 {
        return Err(ZipViewError::CrcMismatch { expected: member.crc32, actual: crc32_actual });
    }

    Ok((MemorySource::from_bytes(label, decoded), facts))
}

/// Materializes a member into a `Decoded` memory source (stored or deflate).
pub fn decode_member<S: ByteSource>(
    label: impl Into<String>,
    source: S,
    member: &ZipMember,
    budget: &Budget,
) -> Result<MemorySource, ZipViewError> {
    let (decoded, _facts) = decode_member_with_facts(label, source, member, budget)?;
    Ok(decoded)
}

fn map_codec_error(error: CodecError) -> ZipViewError {
    if error.is_resource_limit() { ZipViewError::BudgetExceeded } else { ZipViewError::DecompressFailed }
}

fn ieee_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            }
            else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Reads and decodes a member payload into an owned buffer.
pub fn read_member_payload<S: ByteSource>(source: S, member: &ZipMember, budget: &Budget) -> Result<Vec<u8>, ZipViewError> {
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
