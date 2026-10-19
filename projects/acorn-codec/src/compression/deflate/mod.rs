//! Raw Deflate algorithm core (RFC 1951).
//!
//! `Deflate` owns the bitstream, block, Huffman, and sliding window rules. It
//! has no header, trailer, or member concept of its own, so callers that need
//! a zlib or gzip envelope use [crate::compression::Zlib] or [crate::compression::Gzip].

mod block;
mod decoder;
mod encoder;
mod huffman;
mod window;

use crate::{error::CodecError, facts::DecodeFacts, limits::CodecLimits};

use super::stream::{CompressionProtocol, StreamDecoder};

pub use self::{
    block::{
        DeflateBlock, DeflateBlockPlan, FixedBlock, StoredBlock, Symbol, write_block, write_fixed_block, write_stored_block,
    },
    encoder::{DeflateEncoder, DeflateLevel},
};

/// Raw Deflate stream algorithm.
#[derive(Debug, Clone, Copy, Default)]
pub struct Deflate;

impl Deflate {
    /// Decodes a raw Deflate stream with the given limits.
    pub fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        decoder::decode_raw(input, limits)
    }

    /// Encodes `input` as a raw Deflate stream.
    pub fn encode(input: &[u8], level: DeflateLevel) -> Vec<u8> {
        DeflateEncoder::new(level).encode(input)
    }

    /// Creates a buffered stream decoder for raw Deflate.
    pub fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Deflate, limits)
    }
}
