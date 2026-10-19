//! Gzip envelope (RFC 1952).
//!
//! `Gzip` owns the member header and its optional fields (`FEXTRA`, `FNAME`,
//! `FCOMMENT`, `FHCRC`), the CRC32 and `ISIZE` trailer, the member boundary, and
//! the trailing-input rule. The Deflate bitstream itself belongs to [`Deflate`],
//! so this module never re-implements it.
//!
//! A decoded stream is **one member**. RFC 1952 allows further members to follow
//! the trailer, so anything after the first member is reported as
//! `trailing_input` instead of being followed, and the caller decides whether a
//! concatenation is a valid input for it.

use crate::{
    compression::deflate::{Deflate, DeflateLevel},
    error::{ChecksumAlgorithm, CodecError, CodecFeature, EnvelopeKind},
    facts::{Canonicality, ChecksumFacts, DecodeFacts},
    limits::CodecLimits,
};

use super::{
    checksum::crc32,
    stream::{CompressionProtocol, EnvelopeFacts, StreamDecoder},
};

/// Fixed member header: deflate, no flags, zero timestamp, `XFL` 0, unknown OS.
///
/// The timestamp and `XFL` stay fixed so `Gzip::encode` is reproducible.
const MEMBER_HEADER: [u8; 10] = [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF];

/// Gzip member flag: an extra field follows the header.
const FLAG_EXTRA: u8 = 0x04;
/// Gzip member flag: an original file name follows, zero terminated.
const FLAG_NAME: u8 = 0x08;
/// Gzip member flag: a comment follows, zero terminated.
const FLAG_COMMENT: u8 = 0x10;
/// Gzip member flag: a header CRC16 follows the optional fields.
const FLAG_HEADER_CRC: u8 = 0x02;
/// Length of the fixed member header; Deflate starts directly after it.
const HEADER_LEN: usize = 10;
/// Length of the trailer: CRC32 and `ISIZE`, both little endian.
const TRAILER_LEN: usize = 8;

/// Gzip stream codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Gzip;

impl Gzip {
    /// Decodes a gzip stream with the given limits.
    pub fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        decode_gzip(input, limits).map(EnvelopeFacts::into_decode)
    }

    /// Encodes `input` as a single-member gzip stream.
    ///
    /// The envelope writes its own header and trailer around the Deflate output:
    /// a fixed member header, `Deflate::encode`, the CRC32, and `ISIZE`.
    pub fn encode(input: &[u8], level: DeflateLevel) -> Vec<u8> {
        let mut output = Vec::with_capacity(input.len() + MEMBER_HEADER.len() + TRAILER_LEN + 1);
        output.extend_from_slice(&MEMBER_HEADER);
        output.extend_from_slice(&Deflate::encode(input, level));
        output.extend_from_slice(&crc32(input).to_le_bytes());
        output.extend_from_slice(&(input.len() as u32).to_le_bytes());
        output
    }

    /// Creates a buffered stream decoder for gzip.
    pub fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Gzip, limits)
    }
}

/// Advances past a zero-terminated header field.
///
/// The terminator is required: an unterminated name or comment is a truncated
/// header, not payload.
fn skip_zero_terminated(input: &[u8], offset: usize) -> Result<usize, CodecError> {
    let end = input[offset..]
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(CodecError::InputExhausted { offset: input.len(), needed: 1 })?;
    Ok(offset + end + 1)
}

/// Decodes a gzip member and reports its envelope facts.
pub(crate) fn decode_gzip(input: &[u8], limits: CodecLimits) -> Result<EnvelopeFacts, CodecError> {
    if input.len() < HEADER_LEN {
        return Err(CodecError::InputExhausted { offset: input.len(), needed: HEADER_LEN - input.len() });
    }

    if input[0] != 0x1F || input[1] != 0x8B {
        return Err(CodecError::InvalidEnvelope { offset: 0, kind: EnvelopeKind::Gzip });
    }
    if input[2] != 8 {
        return Err(CodecError::UnsupportedFeature { feature: CodecFeature::CompressionMethod(input[2]) });
    }

    let flags = input[3];
    let mut pos = HEADER_LEN;

    if flags & FLAG_EXTRA != 0 {
        if input.len() < pos + 2 {
            return Err(CodecError::InputExhausted { offset: input.len(), needed: pos + 2 - input.len() });
        }
        let extra_len = usize::from(u16::from_le_bytes([input[pos], input[pos + 1]]));
        pos += 2;
        if input.len() < pos + extra_len {
            return Err(CodecError::InputExhausted { offset: input.len(), needed: pos + extra_len - input.len() });
        }
        pos += extra_len;
    }
    if flags & FLAG_NAME != 0 {
        pos = skip_zero_terminated(input, pos)?;
    }
    if flags & FLAG_COMMENT != 0 {
        pos = skip_zero_terminated(input, pos)?;
    }
    if flags & FLAG_HEADER_CRC != 0 {
        if input.len() < pos + 2 {
            return Err(CodecError::InputExhausted { offset: input.len(), needed: pos + 2 - input.len() });
        }
        // RFC 1952: the field is the two least significant bytes of the CRC32 of
        // every header byte before the field itself.
        let expected = u16::from_le_bytes([input[pos], input[pos + 1]]);
        let actual = crc32(&input[..pos]) as u16;
        if expected != actual {
            return Err(CodecError::ChecksumMismatch {
                algorithm: ChecksumAlgorithm::Crc32,
                expected: u64::from(expected),
                actual: u64::from(actual),
            });
        }
        pos += 2;
    }

    if pos >= input.len() {
        return Err(CodecError::InputExhausted { offset: pos, needed: 1 });
    }

    let facts = Deflate::decode(&input[pos..], limits)?;
    let trailer_start = pos + facts.input_consumed;

    if input.len() < trailer_start + TRAILER_LEN {
        return Err(CodecError::InputExhausted { offset: input.len(), needed: trailer_start + TRAILER_LEN - input.len() });
    }

    let expected_crc = u32::from_le_bytes([
        input[trailer_start],
        input[trailer_start + 1],
        input[trailer_start + 2],
        input[trailer_start + 3],
    ]);
    let actual_crc = crc32(&facts.value);
    if expected_crc != actual_crc {
        return Err(CodecError::ChecksumMismatch {
            algorithm: ChecksumAlgorithm::Crc32,
            expected: u64::from(expected_crc),
            actual: u64::from(actual_crc),
        });
    }

    let expected_size = u32::from_le_bytes([
        input[trailer_start + 4],
        input[trailer_start + 5],
        input[trailer_start + 6],
        input[trailer_start + 7],
    ]);
    let actual_size = facts.output_produced as u32;
    if expected_size != actual_size {
        return Err(CodecError::InvalidEnvelope { offset: trailer_start as u64 + 4, kind: EnvelopeKind::Gzip });
    }

    let total_consumed = trailer_start + TRAILER_LEN;
    Ok(EnvelopeFacts::new(
        DecodeFacts {
            value: facts.value,
            input_consumed: total_consumed,
            output_produced: facts.output_produced,
            complete: true,
            trailing_input: input.len() - total_consumed,
            canonical: Canonicality::NotApplicable,
        },
        ChecksumFacts { algorithm: ChecksumAlgorithm::Crc32, value: u64::from(actual_crc), verified: true },
    ))
}
