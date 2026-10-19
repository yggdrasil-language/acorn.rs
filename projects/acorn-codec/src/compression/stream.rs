//! Stream decoding over compression protocols.
//!
//! Both entries share one algorithm: [`StreamCodec::decode`] is the in-memory
//! convenience form of pushing every chunk into a [`StreamDecoder`] and calling
//! [`StreamDecoder::finish`]. The decoders here buffer input until `finish`
//! because Deflate needs a complete bitstream, so no envelope owns a second
//! implementation.

use crate::{
    error::CodecError,
    facts::{DecodeFacts, StreamFacts},
    limits::CodecLimits,
};

use super::{
    deflate::Deflate,
    envelope::{Gzip, Zlib},
};

/// Compression protocol a stream decoder expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionProtocol {
    /// Raw Deflate without an envelope (RFC 1951).
    RawDeflate,
    /// Zlib envelope (RFC 1950).
    Zlib,
    /// Gzip envelope (RFC 1952).
    Gzip,
}

/// Buffered stream decoder for one compression protocol.
#[derive(Debug, Clone)]
pub struct StreamDecoder {
    protocol: CompressionProtocol,
    limits: CodecLimits,
    input: Vec<u8>,
    facts: Option<StreamFacts>,
}

impl StreamDecoder {
    /// Creates a decoder for `protocol`.
    pub fn new(protocol: CompressionProtocol, limits: CodecLimits) -> Self {
        Self { protocol, limits, input: Vec::new(), facts: None }
    }

    /// Returns the protocol this decoder expects.
    #[inline]
    pub const fn protocol(&self) -> CompressionProtocol {
        self.protocol
    }

    /// Buffers another input chunk.
    ///
    /// Failing chunks are reported by [`StreamDecoder::finish`], which owns the
    /// error position for the whole bitstream.
    pub fn push(&mut self, chunk: &[u8]) {
        self.input.extend_from_slice(chunk);
    }

    /// Decodes the buffered input, appends the result to `output`, and records facts.
    pub fn finish(&mut self, output: &mut Vec<u8>) -> Result<&StreamFacts, CodecError> {
        let facts = match self.protocol {
            CompressionProtocol::RawDeflate => Deflate::decode(&self.input, self.limits)?,
            CompressionProtocol::Zlib => super::envelope::zlib::decode_zlib(&self.input, self.limits)?,
            CompressionProtocol::Gzip => super::envelope::gzip::decode_gzip(&self.input, self.limits)?,
        };
        output.extend_from_slice(&facts.value);
        self.facts = Some(StreamFacts {
            input_consumed: facts.input_consumed as u64,
            output_produced: facts.output_produced as u64,
            complete: facts.complete,
            trailing_input: facts.trailing_input as u64,
            checksum: None,
        });
        Ok(self.facts.as_ref().expect("facts were just recorded"))
    }

    /// Returns the facts recorded by [`StreamDecoder::finish`].
    ///
    /// Envelope checksum values are not surfaced here yet: a failed trailer is
    /// reported as [`CodecError::ChecksumMismatch`] instead.
    #[inline]
    pub fn facts(&self) -> Option<&StreamFacts> {
        self.facts.as_ref()
    }
}

/// Compression entry that supports stream decoding.
pub trait StreamCodec {
    /// Creates a buffered stream decoder for this protocol.
    fn decoder(limits: CodecLimits) -> StreamDecoder;

    /// Decodes a complete buffer through the streaming path.
    fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError>;
}

impl StreamCodec for Deflate {
    fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::RawDeflate, limits)
    }

    fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        Deflate::decode(input, limits)
    }
}

impl StreamCodec for Zlib {
    fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Zlib, limits)
    }

    fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        super::envelope::zlib::decode_zlib(input, limits)
    }
}

impl StreamCodec for Gzip {
    fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Gzip, limits)
    }

    fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        super::envelope::gzip::decode_gzip(input, limits)
    }
}
