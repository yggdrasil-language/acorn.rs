//! Stream decoding over compression protocols.
//!
//! Both entries share one algorithm: [`StreamCodec::decode`] is the in-memory
//! convenience form of pushing every chunk into a [`StreamDecoder`] and calling
//! [`StreamDecoder::finish`]. The decoders here buffer input until `finish`
//! because Deflate needs a complete bitstream, so no protocol owns a second
//! implementation. Buffering is bounded by `max_input_bytes` and the decoder
//! reports its [`StreamState`], so a buffered interface is never presented as a
//! resumable mid-bitstream one.
//!
//! Only three protocols exist, matching the three public entries in
//! [`crate::compression`]: `Deflate` (the RFC 1951 algorithm), `Zlib` and `Gzip`
//! (its RFC 1950 and RFC 1952 envelopes).

use crate::{
    error::CodecError,
    facts::{ChecksumFacts, DecodeFacts, StreamFacts, StreamState},
    limits::CodecLimits,
};

use super::{deflate::Deflate, gzip::Gzip, zlib::Zlib};

/// Compression protocol a stream decoder expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionProtocol {
    /// Deflate without an envelope (RFC 1951).
    Deflate,
    /// Zlib envelope (RFC 1950).
    Zlib,
    /// Gzip envelope (RFC 1952).
    Gzip,
}

/// One-shot envelope decode plus the trailer validation it performed.
///
/// Format adapters that need the checksum value read it from here, and the
/// stream path forwards it into [`StreamFacts`] instead of dropping it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvelopeFacts {
    decode: DecodeFacts<Vec<u8>>,
    checksum: Option<ChecksumFacts>,
}

impl EnvelopeFacts {
    /// Pairs decode facts with the checksum the envelope verified, if any.
    pub(crate) fn new(decode: DecodeFacts<Vec<u8>>, checksum: ChecksumFacts) -> Self {
        Self { decode, checksum: Some(checksum) }
    }

    /// Decode facts with no envelope checksum, used by the Deflate protocol.
    pub(crate) fn without_checksum(decode: DecodeFacts<Vec<u8>>) -> Self {
        Self { decode, checksum: None }
    }

    /// Consumes the pair and returns the one-shot decode facts.
    #[inline]
    pub(crate) fn into_decode(self) -> DecodeFacts<Vec<u8>> {
        self.decode
    }

    /// Consumes the pair and returns both parts.
    #[inline]
    pub(crate) fn into_parts(self) -> (DecodeFacts<Vec<u8>>, Option<ChecksumFacts>) {
        (self.decode, self.checksum)
    }
}

/// Buffered stream decoder for one compression protocol.
#[derive(Debug, Clone)]
pub struct StreamDecoder {
    protocol: CompressionProtocol,
    limits: CodecLimits,
    input: Vec<u8>,
    facts: Option<StreamFacts>,
    state: StreamState,
}

impl StreamDecoder {
    /// Creates a decoder for `protocol`.
    pub fn new(protocol: CompressionProtocol, limits: CodecLimits) -> Self {
        Self { protocol, limits, input: Vec::new(), facts: None, state: StreamState::Created }
    }

    /// Returns the protocol this decoder expects.
    #[inline]
    pub const fn protocol(&self) -> CompressionProtocol {
        self.protocol
    }

    /// Returns the lifecycle state of this decoder.
    #[inline]
    pub const fn state(&self) -> StreamState {
        self.state
    }

    /// Buffers another input chunk.
    ///
    /// The chunk is accepted only while the decoder has neither completed nor
    /// failed, and only when the buffered input stays inside `max_input_bytes`. A
    /// rejected chunk leaves the decoder unchanged, so the caller keeps ownership
    /// of the decision to fail or to release buffered input.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), CodecError> {
        match self.state {
            StreamState::Complete | StreamState::Failed => {
                return Err(CodecError::InvalidStreamState { state: self.state });
            }
            StreamState::Created | StreamState::Receiving => {}
        }
        let limit = self.limits.max_input_bytes.unwrap_or(u64::MAX);
        let buffered = self.input.len() as u64;
        let offered = chunk.len() as u64;
        let total = buffered.checked_add(offered).ok_or(CodecError::InputLimitExceeded { buffered, offered, limit })?;
        if total > limit {
            return Err(CodecError::InputLimitExceeded { buffered, offered, limit });
        }
        self.input.extend_from_slice(chunk);
        self.state = StreamState::Receiving;
        Ok(())
    }

    /// Decodes the buffered input, appends the result to `output`, and records facts.
    ///
    /// A successful decode moves the decoder to `Complete`. A failed decode moves
    /// it to `Failed` and keeps that state, so a failed bitstream cannot be replayed
    /// and a second `finish` is rejected instead of reporting stale facts.
    pub fn finish(&mut self, output: &mut Vec<u8>) -> Result<&StreamFacts, CodecError> {
        match self.state {
            StreamState::Complete | StreamState::Failed => {
                return Err(CodecError::InvalidStreamState { state: self.state });
            }
            StreamState::Created | StreamState::Receiving => {}
        }
        let decoded = match self.protocol {
            CompressionProtocol::Deflate => Deflate::decode(&self.input, self.limits).map(EnvelopeFacts::without_checksum),
            CompressionProtocol::Zlib => super::zlib::decode_zlib(&self.input, self.limits),
            CompressionProtocol::Gzip => super::gzip::decode_gzip(&self.input, self.limits),
        };
        let envelope = match decoded {
            Ok(envelope) => envelope,
            Err(error) => {
                self.state = StreamState::Failed;
                return Err(error);
            }
        };
        let (decode, checksum) = envelope.into_parts();
        output.extend_from_slice(&decode.value);
        self.facts = Some(StreamFacts {
            input_consumed: decode.input_consumed as u64,
            output_produced: decode.output_produced as u64,
            complete: decode.complete,
            trailing_input: decode.trailing_input as u64,
            checksum,
        });
        self.state = StreamState::Complete;
        Ok(self.facts.as_ref().expect("facts were just recorded"))
    }

    /// Returns the facts recorded by [`StreamDecoder::finish`].
    ///
    /// The checksum is `None` for the Deflate protocol, which has no trailer,
    /// and carries the verified trailer value for the zlib and gzip envelopes.
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
        StreamDecoder::new(CompressionProtocol::Deflate, limits)
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
        super::zlib::decode_zlib(input, limits).map(EnvelopeFacts::into_decode)
    }
}

impl StreamCodec for Gzip {
    fn decoder(limits: CodecLimits) -> StreamDecoder {
        StreamDecoder::new(CompressionProtocol::Gzip, limits)
    }

    fn decode(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        super::gzip::decode_gzip(input, limits).map(EnvelopeFacts::into_decode)
    }
}
