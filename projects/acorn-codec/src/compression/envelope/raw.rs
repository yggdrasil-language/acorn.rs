//! Raw Deflate envelope.

use crate::{
    compression::{
        deflate::Deflate,
        stream::{CompressionProtocol, StreamDecoder},
    },
    error::CodecError,
    facts::DecodeFacts,
    limits::CodecLimits,
};

/// Raw Deflate stream entry.
///
/// Formats that state the compression method explicitly, such as ZIP member
/// method 8 or a PDF `FlateDecode` filter, decode through this entry instead of
/// letting the codec probe for an envelope.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawDeflate;

impl RawDeflate {
    /// Decodes a raw Deflate stream with the given limits.
    pub fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        Deflate::decode(input, limits)
    }

    /// Creates a buffered stream decoder for raw Deflate.
    pub fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::RawDeflate, limits)
    }
}
