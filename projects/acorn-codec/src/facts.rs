//! Decode result facts shared across transform families.

/// Whether an encoding is in canonical form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canonicality {
    /// Encoding is canonical.
    Canonical,
    /// Encoding is valid but non-canonical.
    NonCanonical,
    /// Canonicality does not apply to this transform.
    NotApplicable,
}

/// Facts produced by a decode operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeFacts<T> {
    /// Decoded value.
    pub value: T,
    /// Input bytes consumed.
    pub input_consumed: usize,
    /// Output bytes produced (for byte-producing transforms).
    pub output_produced: usize,
    /// Whether the transform completed successfully.
    pub complete: bool,
    /// Trailing input bytes after a complete decode.
    pub trailing_input: usize,
    /// Canonicality of the input encoding.
    pub canonical: Canonicality,
}

impl<T> DecodeFacts<T> {
    /// Whether the entire input slice was consumed with no trailing bytes.
    #[inline]
    pub fn fully_consumed(&self, total_input: usize) -> bool {
        self.complete && self.trailing_input == 0 && self.input_consumed == total_input
    }

    /// Whether a raw DEFLATE stream completed with at most one partial final byte left.
    ///
    /// Huffman blocks may leave 0-7 padding bits in the last byte. Some template
    /// encoders also pack the next block header into the same byte.
    #[inline]
    pub fn deflate_stream_complete(&self, total_input: usize) -> bool {
        self.complete && self.input_consumed <= total_input && total_input - self.input_consumed <= 1
    }
}

/// Checksum facts from envelope validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChecksumFacts {
    /// Algorithm that produced `value`.
    pub algorithm: crate::error::ChecksumAlgorithm,
    /// Computed checksum value.
    pub value: u64,
    /// Whether the checksum matched the trailer.
    pub verified: bool,
}

/// Facts from a stream decode (compression envelopes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamFacts {
    /// Total input bytes consumed.
    pub input_consumed: u64,
    /// Total output bytes produced.
    pub output_produced: u64,
    /// Whether the stream completed (including trailer validation).
    pub complete: bool,
    /// Trailing input after stream end.
    pub trailing_input: u64,
    /// Checksum validation result, if applicable.
    pub checksum: Option<ChecksumFacts>,
}

impl StreamFacts {
    /// Whether the entire input was consumed.
    #[inline]
    pub fn fully_consumed(&self, total_input: usize) -> bool {
        self.complete && self.trailing_input == 0 && self.input_consumed == total_input as u64
    }
}

/// Lifecycle state of a buffered stream decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// No input chunk has been accepted yet.
    Created,
    /// Input chunks are buffered and no decode has run yet.
    Receiving,
    /// A decode completed and recorded facts. No further input is accepted.
    Complete,
    /// A decode failed. The decoder keeps the failure state and accepts no further input.
    Failed,
}

/// Byte decode result with attached facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteDecode {
    /// Decoded bytes.
    bytes: Vec<u8>,
    /// Decode facts.
    facts: DecodeFacts<()>,
}

impl ByteDecode {
    /// Creates a byte decode result.
    pub fn new(bytes: Vec<u8>, facts: DecodeFacts<()>) -> Self {
        Self { bytes, facts }
    }

    /// Returns the decoded bytes.
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns decode facts.
    #[inline]
    pub fn facts(&self) -> &DecodeFacts<()> {
        &self.facts
    }

    /// Consumes the wrapper and returns the decoded bytes.
    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
