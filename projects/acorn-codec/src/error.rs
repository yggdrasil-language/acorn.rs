//! Codec errors independent of format adapters and parse budgets.

use crate::bit::BitPosition;

/// Expansion ratio limit expressed as output/input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio {
    /// Numerator of the ratio.
    pub numerator: u64,
    /// Denominator of the ratio.
    pub denominator: u64,
}

impl Ratio {
    /// Creates a ratio of `numerator` / `denominator`.
    #[inline]
    pub const fn new(numerator: u64, denominator: u64) -> Self {
        Self { numerator, denominator }
    }
}

/// Checksum algorithm identifier for envelope validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    /// Adler-32 (zlib).
    Adler32,
    /// CRC-32 (gzip).
    Crc32,
}

/// Envelope kind for compression wrapper errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeKind {
    /// Zlib wrapper (RFC 1950).
    Zlib,
    /// Gzip wrapper (RFC 1952).
    Gzip,
}

/// Unsupported codec feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecFeature {
    /// Reserved or unknown compression method.
    CompressionMethod(u8),
    /// Unsupported Huffman block type.
    BlockType(u8),
}

/// Errors raised by representation transforms in `acorn-codec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Input ended before the transform could complete.
    InputExhausted {
        /// Byte offset where more input was needed.
        offset: usize,
        /// Minimum additional bytes required.
        needed: usize,
    },
    /// A byte violated the expected alphabet or encoding.
    InvalidByte {
        /// Byte offset of the violation.
        offset: usize,
        /// The offending byte.
        byte: u8,
    },
    /// Padding rules were violated.
    InvalidPadding {
        /// Byte offset of the padding violation.
        offset: usize,
    },
    /// A bit pattern was invalid for the current context.
    InvalidBitPattern {
        /// Byte offset at the time of the violation.
        offset: usize,
        /// Width of the invalid bit pattern.
        width: u8,
    },
    /// An integer exceeded the allowed bit width.
    IntegerOverflow {
        /// Byte offset where overflow was detected.
        offset: usize,
        /// Maximum allowed bit width.
        width: u32,
    },
    /// Encoding is valid but not in canonical form.
    NonCanonicalEncoding {
        /// Byte offset of the non-canonical encoding.
        offset: usize,
    },
    /// Output would exceed the caller-provided limit.
    OutputLimitExceeded {
        /// Bytes produced before the limit was hit.
        produced: u64,
        /// Output limit that was exceeded.
        limit: u64,
    },
    /// Expansion ratio limit was exceeded.
    ExpansionRatioExceeded {
        /// Input bytes consumed.
        input: u64,
        /// Output bytes produced.
        output: u64,
        /// Ratio limit that was exceeded.
        limit: Ratio,
    },
    /// Deflate block header was invalid.
    InvalidBlockHeader {
        /// Bit position of the block header.
        position: BitPosition,
    },
    /// Huffman tree construction or decoding failed.
    InvalidHuffmanTree {
        /// Bit position where the tree was invalid.
        position: BitPosition,
    },
    /// Huffman symbol was out of range for the current table.
    HuffmanSymbolOutOfRange {
        /// Bit position of the symbol.
        position: BitPosition,
        /// Decoded symbol value.
        symbol: u16,
    },
    /// Back-reference distance exceeded the sliding window.
    DistanceOutOfWindow {
        /// Bit position of the reference.
        position: BitPosition,
        /// Requested distance.
        distance: u32,
        /// Available window size.
        window: u32,
    },
    /// Back-reference could not be satisfied.
    InvalidBackReference {
        /// Bit position of the reference.
        position: BitPosition,
        /// Requested distance.
        distance: u32,
        /// Bytes available at the reference point.
        available: u32,
    },
    /// Envelope checksum did not match.
    ChecksumMismatch {
        /// Checksum algorithm.
        algorithm: ChecksumAlgorithm,
        /// Expected checksum value.
        expected: u64,
        /// Computed checksum value.
        actual: u64,
    },
    /// Compression envelope header or trailer was invalid.
    InvalidEnvelope {
        /// Byte offset of the envelope error.
        offset: u64,
        /// Envelope kind.
        kind: EnvelopeKind,
    },
    /// Trailing input remains after a complete decode.
    TrailingInput {
        /// Byte offset where trailing input begins.
        offset: u64,
        /// Length of trailing input.
        length: u64,
    },
    /// Feature is not supported by this build or profile.
    UnsupportedFeature {
        /// Unsupported feature identifier.
        feature: CodecFeature,
    },
    /// Internal invariant was violated (should not occur on valid inputs).
    InternalInvariantViolation,
}

impl CodecError {
    /// Whether the error reports a resource limit imposed by the caller.
    ///
    /// Format adapters map these to budget diagnostics while malformed,
    /// truncated, or unsupported input maps to representation and
    /// decompression failures.
    #[inline]
    pub const fn is_resource_limit(&self) -> bool {
        matches!(self, Self::OutputLimitExceeded { .. } | Self::ExpansionRatioExceeded { .. })
    }
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputExhausted { offset, needed } => {
                write!(f, "codec input exhausted at offset {offset}, needed {needed} more byte(s)")
            }
            Self::InvalidByte { offset, byte } => {
                write!(f, "codec invalid byte 0x{byte:02X} at offset {offset}")
            }
            Self::InvalidPadding { offset } => {
                write!(f, "codec invalid padding at offset {offset}")
            }
            Self::InvalidBitPattern { offset, width } => {
                write!(f, "codec invalid {width}-bit pattern at offset {offset}")
            }
            Self::IntegerOverflow { offset, width } => {
                write!(f, "codec integer overflow at offset {offset} (width {width})")
            }
            Self::NonCanonicalEncoding { offset } => {
                write!(f, "codec non-canonical encoding at offset {offset}")
            }
            Self::OutputLimitExceeded { produced, limit } => {
                write!(f, "codec output limit exceeded: produced {produced}, limit {limit}")
            }
            Self::ExpansionRatioExceeded { input, output, limit } => {
                write!(f, "codec expansion ratio exceeded: {output}/{input} > {}/{}", limit.numerator, limit.denominator)
            }
            Self::InvalidBlockHeader { position } => {
                write!(f, "codec invalid deflate block header at {position}")
            }
            Self::InvalidHuffmanTree { position } => {
                write!(f, "codec invalid Huffman tree at {position}")
            }
            Self::HuffmanSymbolOutOfRange { position, symbol } => {
                write!(f, "codec Huffman symbol {symbol} out of range at {position}")
            }
            Self::DistanceOutOfWindow { position, distance, window } => {
                write!(f, "codec distance {distance} out of window {window} at {position}")
            }
            Self::InvalidBackReference { position, distance, available } => {
                write!(f, "codec invalid back-reference distance {distance} (available {available}) at {position}")
            }
            Self::ChecksumMismatch { algorithm, expected, actual } => {
                write!(f, "codec checksum mismatch ({algorithm:?}): expected {expected:#x}, actual {actual:#x}")
            }
            Self::InvalidEnvelope { offset, kind } => {
                write!(f, "codec invalid {kind:?} envelope at offset {offset}")
            }
            Self::TrailingInput { offset, length } => {
                write!(f, "codec trailing input: {length} byte(s) at offset {offset}")
            }
            Self::UnsupportedFeature { feature } => {
                write!(f, "codec unsupported feature: {feature:?}")
            }
            Self::InternalInvariantViolation => f.write_str("codec internal invariant violation"),
        }
    }
}

impl std::error::Error for CodecError {}
