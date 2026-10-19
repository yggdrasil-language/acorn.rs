//! Gzip envelope decode (RFC 1952).

use crate::{
    compression::deflate::Deflate,
    error::{ChecksumAlgorithm, CodecError, EnvelopeKind},
    facts::{Canonicality, DecodeFacts},
    limits::CodecLimits,
};

use super::super::{
    checksum::crc32,
    stream::{CompressionProtocol, StreamDecoder},
};

/// Gzip stream codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Gzip;

impl Gzip {
    /// Decodes a gzip stream with the given limits.
    pub fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        decode_gzip(input, limits)
    }

    /// Creates a buffered stream decoder for gzip.
    pub fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Gzip, limits)
    }
}

pub(crate) fn decode_gzip(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
    if input.len() < 10 {
        return Err(CodecError::InputExhausted { offset: input.len(), needed: 10 - input.len() });
    }

    if input[0] != 0x1F || input[1] != 0x8B {
        return Err(CodecError::InvalidEnvelope { offset: 0, kind: EnvelopeKind::Gzip });
    }
    if input[2] != 8 {
        return Err(CodecError::UnsupportedFeature { feature: crate::error::CodecFeature::CompressionMethod(input[2]) });
    }

    let flags = input[3];
    let mut pos = 10usize;

    if flags & 0x04 != 0 {
        if input.len() < pos + 2 {
            return Err(CodecError::InputExhausted { offset: input.len(), needed: 2 });
        }
        let extra_len = u16::from_le_bytes([input[pos], input[pos + 1]]) as usize;
        pos += 2 + extra_len;
    }
    if flags & 0x08 != 0 {
        while pos < input.len() && input[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    if flags & 0x10 != 0 {
        while pos < input.len() && input[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    if flags & 0x02 != 0 {
        pos += 2;
    }

    if pos >= input.len() {
        return Err(CodecError::InputExhausted { offset: pos, needed: 1 });
    }

    let deflate_input = &input[pos..];
    let facts = Deflate::decode(deflate_input, limits)?;
    let consumed_in_deflate = facts.input_consumed;
    let trailer_start = pos + consumed_in_deflate;

    if input.len() < trailer_start + 8 {
        return Err(CodecError::InputExhausted { offset: input.len(), needed: trailer_start + 8 - input.len() });
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
            expected: expected_crc as u64,
            actual: actual_crc as u64,
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

    let total_consumed = trailer_start + 8;
    let trailing = input.len() - total_consumed;

    Ok(DecodeFacts {
        value: facts.value,
        input_consumed: total_consumed,
        output_produced: facts.output_produced,
        complete: true,
        trailing_input: trailing,
        canonical: Canonicality::NotApplicable,
    })
}

/// Encodes `data` as a minimal gzip stream using a stored Deflate block (for tests).
pub fn encode_stored(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(18 + data.len());
    output.extend_from_slice(&[0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF]);
    output.extend_from_slice(&super::super::deflate::encode_stored(data));
    let checksum = crc32(data);
    output.extend_from_slice(&checksum.to_le_bytes());
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output
}
