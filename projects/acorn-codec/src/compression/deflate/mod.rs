//! Raw Deflate algorithm core (RFC 1951).
//!
//! `Deflate` owns the bitstream, block, Huffman, and sliding window rules. It
//! has no header, trailer, or member concept of its own, so callers that need
//! an envelope use [`crate::compression::envelope`].

mod decoder;
mod huffman;
mod window;

use crate::{error::CodecError, facts::DecodeFacts, limits::CodecLimits};

use super::stream::{CompressionProtocol, StreamDecoder};

/// Raw Deflate stream algorithm.
#[derive(Debug, Clone, Copy, Default)]
pub struct Deflate;

impl Deflate {
    /// Decodes a raw Deflate stream with the given limits.
    pub fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        decoder::decode_raw(input, limits)
    }

    /// Creates a buffered stream decoder for raw Deflate.
    pub fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::RawDeflate, limits)
    }
}

/// Encodes `data` as a single stored Deflate block (used by envelope writers and tests).
pub fn encode_stored(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(5 + data.len());
    output.push(0x01);
    let len = data.len() as u16;
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(&(len ^ 0xFFFF).to_le_bytes());
    output.extend_from_slice(data);
    output
}
