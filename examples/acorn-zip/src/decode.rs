use acorn_codec::{CodecError, CodecLimits, compression::Deflate};
use acorn_core::{Budget, BudgetError, BudgetTracker};
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

/// Which ledger an open hold occupies.
enum HoldKind {
    /// Bytes that will be read from the source.
    ReadBytes,
    /// Bytes that will be produced by decoding.
    DecodedBytes,
}

/// Keeps one ledger reservation open for a single step.
///
/// Bytes the step did not settle are released when the hold leaves scope, so a failed
/// or cancelled step cannot leak its allowance.
struct Hold<'a> {
    ledger: &'a mut BudgetTracker,
    kind: HoldKind,
    outstanding: u64,
}

impl<'a> Hold<'a> {
    /// Reserves `bytes` in the ledger for this step.
    fn begin(ledger: &'a mut BudgetTracker, kind: HoldKind, bytes: u64) -> Result<Self, ZipViewError> {
        let reserved = match kind {
            HoldKind::ReadBytes => ledger.reserve_read_bytes(bytes),
            HoldKind::DecodedBytes => ledger.reserve_decoded_bytes(bytes),
        };
        reserved.map_err(map_budget_error)?;
        Ok(Self { ledger, kind, outstanding: bytes })
    }

    /// Records what the step actually consumed, clamped to what it reserved.
    fn settle(&mut self, actual: u64) -> Result<(), ZipViewError> {
        let settled = actual.min(self.outstanding);
        let settled_in_ledger = match self.kind {
            HoldKind::ReadBytes => self.ledger.settle_read_bytes(settled),
            HoldKind::DecodedBytes => self.ledger.settle_decoded_bytes(settled),
        };
        settled_in_ledger.map_err(map_budget_error)?;
        self.outstanding -= settled;
        Ok(())
    }
}

impl Drop for Hold<'_> {
    fn drop(&mut self) {
        if self.outstanding == 0 {
            return;
        }
        match self.kind {
            HoldKind::ReadBytes => {
                let _ = self.ledger.release_read_bytes(self.outstanding);
            }
            HoldKind::DecodedBytes => {
                let _ = self.ledger.release_decoded_bytes(self.outstanding);
            }
        }
    }
}

/// Materializes a member and returns decode facts (CRC / consumption).
///
/// The declared read and decoded sizes are reserved in `ledger` before the payload is
/// read, and settled with the bytes actually consumed. Holding and settling through one
/// ledger means every member of a document shares a single allowance instead of a fresh
/// limit per member.
pub fn decode_member_with_facts<S: ByteSource>(
    label: impl Into<String>,
    source: S,
    member: &ZipMember,
    ledger: &mut BudgetTracker,
) -> Result<(MemorySource, MemberDecodeFacts), ZipViewError> {
    check_declared_extent(member)?;
    let limits = *ledger.limits();

    let compressed = {
        let mut hold = Hold::begin(ledger, HoldKind::ReadBytes, member.compressed_size)?;
        let compressed = read_raw_payload(source, member)?;
        hold.settle(compressed.len() as u64)?;
        if compressed.len() as u64 != member.compressed_size {
            return Err(ZipViewError::SizeMismatch);
        }
        compressed
    };
    let budget_read_bytes = compressed.len() as u64;

    let (decoded, fully_consumed) = {
        let mut hold = Hold::begin(ledger, HoldKind::DecodedBytes, member.uncompressed_size)?;
        let decoded = match member.compression_method {
            COMPRESSION_STORED => {
                if member.uncompressed_size != member.compressed_size {
                    return Err(ZipViewError::SizeMismatch);
                }
                (compressed, true)
            }
            COMPRESSION_DEFLATE => decode_deflate_member(compressed, member, &limits)?,
            other => return Err(ZipViewError::UnsupportedCompression(other)),
        };
        hold.settle(decoded.0.len() as u64)?;
        if decoded.0.len() as u64 != member.uncompressed_size {
            return Err(ZipViewError::SizeMismatch);
        }
        decoded
    };

    let budget_decoded_bytes = decoded.len() as u64;
    // The decoded bytes stay live inside the returned source, so they are retained.
    ledger.reserve_cache_bytes(budget_decoded_bytes).map_err(map_budget_error)?;

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
    ledger: &mut BudgetTracker,
) -> Result<MemorySource, ZipViewError> {
    let (decoded, _facts) = decode_member_with_facts(label, source, member, ledger)?;
    Ok(decoded)
}

/// Rejects a member whose declared payload range does not match its declared size.
///
/// Limits are enforced by the reservation each step takes before it reads or allocates.
fn check_declared_extent(member: &ZipMember) -> Result<(), ZipViewError> {
    if member.payload_range.len != member.compressed_size {
        return Err(ZipViewError::SizeMismatch);
    }
    Ok(())
}

/// Decodes a raw Deflate member payload into bytes plus consumption evidence.
fn decode_deflate_member(compressed: Vec<u8>, member: &ZipMember, limits: &Budget) -> Result<(Vec<u8>, bool), ZipViewError> {
    if member.compressed_size == 0 {
        // A zero-length member must also declare zero decoded bytes. Letting the
        // declared value `0` skip the size comparison made an empty declaration
        // accept any decoded extent.
        if member.uncompressed_size != 0 {
            return Err(ZipViewError::SizeMismatch);
        }
        return Ok((Vec::new(), true));
    }
    let limits = CodecLimits::new().with_max_output_bytes(limits.max_decoded_bytes);
    let facts = Deflate::decode(&compressed, limits).map_err(map_codec_error)?;
    let fully_consumed = facts.deflate_stream_complete(compressed.len());
    Ok((facts.value, fully_consumed))
}

/// Maps a codec failure to a member-view failure without discarding its category.
fn map_codec_error(error: CodecError) -> ZipViewError {
    if error.is_resource_limit() { ZipViewError::BudgetExceeded } else { ZipViewError::Codec(error) }
}

/// Maps a ledger refusal to the member-view budget failure.
fn map_budget_error(_: BudgetError) -> ZipViewError {
    ZipViewError::BudgetExceeded
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
pub fn read_member_payload<S: ByteSource>(
    source: S,
    member: &ZipMember,
    ledger: &mut BudgetTracker,
) -> Result<Vec<u8>, ZipViewError> {
    let decoded = decode_member("zip-member", source, member, ledger)?;
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
