//! Zlib envelope decode (RFC 1950).

use crate::{
    compression::deflate::Deflate,
    error::{ChecksumAlgorithm, CodecError, EnvelopeKind},
    facts::{Canonicality, DecodeFacts},
    limits::CodecLimits,
};

use super::super::{
    checksum::adler32,
    stream::{CompressionProtocol, StreamDecoder},
};

/// Zlib stream codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Zlib;

impl Zlib {
    /// Decodes a zlib stream with the given limits.
    pub fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        decode_zlib(input, limits)
    }

    /// Creates a buffered stream decoder for zlib.
    pub fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Zlib, limits)
    }
}

pub(crate) fn decode_zlib(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
    if input.len() < 6 {
        return Err(CodecError::InputExhausted { offset: input.len(), needed: 6 - input.len() });
    }

    let cmf = input[0];
    let flg = input[1];
    if (cmf as u16 * 256 + flg as u16) % 31 != 0 {
        return Err(CodecError::InvalidEnvelope { offset: 0, kind: EnvelopeKind::Zlib });
    }
    if cmf & 0x0F != 8 {
        return Err(CodecError::UnsupportedFeature { feature: crate::error::CodecFeature::CompressionMethod(cmf & 0x0F) });
    }

    let deflate_input = &input[2..];
    let facts = Deflate::decode(deflate_input, limits)?;
    let consumed_in_deflate = facts.input_consumed;
    let trailer_start = 2 + consumed_in_deflate;

    if input.len() < trailer_start + 4 {
        return Err(CodecError::InputExhausted { offset: input.len(), needed: trailer_start + 4 - input.len() });
    }

    let expected = u32::from_be_bytes([
        input[trailer_start],
        input[trailer_start + 1],
        input[trailer_start + 2],
        input[trailer_start + 3],
    ]);
    let actual = adler32(&facts.value);
    if expected != actual {
        return Err(CodecError::ChecksumMismatch {
            algorithm: ChecksumAlgorithm::Adler32,
            expected: expected as u64,
            actual: actual as u64,
        });
    }

    let total_consumed = trailer_start + 4;
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

/// Encodes `data` as a minimal zlib stream using a stored Deflate block (for tests).
pub fn encode_stored(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(6 + data.len());
    output.push(0x78);
    output.push(0x01);
    output.extend_from_slice(&super::super::deflate::encode_stored(data));
    let checksum = adler32(data);
    output.extend_from_slice(&checksum.to_be_bytes());
    output
}
