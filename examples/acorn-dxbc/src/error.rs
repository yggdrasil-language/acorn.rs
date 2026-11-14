//! Structural failures and the byte evidence they keep.

use std::fmt::{Display, Formatter};

use acorn_core::{BudgetError, ByteRange};

/// Byte coordinates for one structural claim.
///
/// `byte_range` is relative to the start of the container. `chunk_index` names
/// the chunk the interval belongs to when the claim is about chunk content, and
/// is absent for header and offset table claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DxbcEvidence {
    /// Index of the chunk the interval belongs to, when it is inside one.
    pub chunk_index: Option<u32>,
    /// Byte interval inside the container.
    pub byte_range: ByteRange,
}

impl DxbcEvidence {
    /// Creates evidence for an interval in the container header or offset table.
    pub const fn container(byte_range: ByteRange) -> Self {
        Self { chunk_index: None, byte_range }
    }

    /// Creates evidence for an interval inside the chunk at `index`.
    pub const fn chunk(index: u32, byte_range: ByteRange) -> Self {
        Self { chunk_index: Some(index), byte_range }
    }

    /// Human-readable location used in error messages.
    fn location(&self) -> String {
        match self.chunk_index {
            Some(index) => format!("chunk {index} at byte {}", self.byte_range.start),
            None => format!("byte {}", self.byte_range.start),
        }
    }
}

/// Structural failure found while mapping a DXBC container.
///
/// Each variant keeps the coordinates of the bytes that produced it, so a
/// consumer can point at the container instead of re-deriving the location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxbcLayoutError {
    /// The source does not report a total length.
    UnknownLength,
    /// Fewer than 32 container bytes are available.
    TruncatedHeader {
        /// Bytes the source reports before the header is complete.
        available_bytes: u64,
        /// Interval that should have held the header.
        evidence: DxbcEvidence,
    },
    /// The leading four bytes are not the `DXBC` magic.
    BadMagic {
        /// Magic bytes that were read.
        magic: [u8; 4],
        /// Interval of the magic bytes.
        evidence: DxbcEvidence,
    },
    /// The declared total length differs from the bytes the source provides.
    LengthMismatch {
        /// Total length declared by the header.
        declared: u32,
        /// Bytes the source reports.
        actual: u64,
        /// Interval of the declared length field.
        evidence: DxbcEvidence,
    },
    /// The declared chunk count needs an offset table that does not fit in the container.
    ChunkCountOutOfBounds {
        /// Chunk count declared by the header.
        chunk_count: u32,
        /// Total length declared by the header.
        declared_length: u64,
        /// Bytes the offset table would occupy.
        table_bytes: u64,
        /// Interval of the chunk count field.
        evidence: DxbcEvidence,
    },
    /// A chunk offset leaves no room for a chunk header inside the container.
    ChunkOffsetOutOfBounds {
        /// Index of the offending offset table entry.
        chunk_index: u32,
        /// Offset that was read.
        offset: u32,
        /// Total length declared by the header.
        declared_length: u64,
        /// Interval of the chunk header the offset should address.
        evidence: DxbcEvidence,
    },
    /// A declared chunk size runs past the end of the container.
    ChunkSizeOutOfBounds {
        /// Index of the offending chunk.
        chunk_index: u32,
        /// FourCC of the offending chunk, as stored.
        four_cc: [u8; 4],
        /// Payload size declared by the chunk header.
        size: u32,
        /// Offset of the chunk header.
        offset: u32,
        /// Total length declared by the header.
        declared_length: u64,
        /// Interval of the declared payload.
        evidence: DxbcEvidence,
    },
    /// Two chunks claim overlapping byte ranges.
    ChunkOverlap {
        /// Index of the chunk that starts first.
        first_index: u32,
        /// Index of the chunk that starts later.
        second_index: u32,
        /// Interval of the later chunk.
        evidence: DxbcEvidence,
    },
    /// A `SHEX`/`SHDR` payload is too small to hold its version and length tokens.
    ShaderChunkTooSmall {
        /// Index of the offending chunk.
        chunk_index: u32,
        /// Payload size declared by the chunk header.
        chunk_size: u32,
        /// Interval of the chunk payload.
        evidence: DxbcEvidence,
    },
    /// A `SHEX`/`SHDR` length token disagrees with the chunk size.
    ///
    /// The token counts 32-bit DWORDs while the chunk header declares bytes, so a
    /// valid chunk satisfies `declared * 4 == chunk_size`.
    ShaderLengthMismatch {
        /// Index of the offending chunk.
        chunk_index: u32,
        /// Length token recorded by the shader chunk, in 32-bit DWORDs.
        declared: u32,
        /// Payload size declared by the chunk header, in bytes.
        chunk_size: u32,
        /// Interval of the length token.
        evidence: DxbcEvidence,
    },
    /// An instruction declares a zero word count.
    ZeroWordCount {
        /// Interval of the instruction's leading word.
        evidence: DxbcEvidence,
    },
    /// An instruction's declared word count runs past the end of the stream.
    InstructionOutOfBounds {
        /// Word count declared by the instruction.
        word_count: u32,
        /// Words that remain in the stream.
        available_words: u64,
        /// Interval of the remaining instruction words.
        evidence: DxbcEvidence,
    },
    /// Bytes after the last complete word do not fill a 32-bit word.
    TrailingPartialWord {
        /// Bytes left after the last complete word.
        remainder_bytes: u64,
        /// Interval of the trailing bytes.
        evidence: DxbcEvidence,
    },
    /// The source returned fewer bytes than requested.
    ShortRead {
        /// Requested byte count.
        expected: u64,
        /// Bytes the source returned.
        actual: u64,
        /// Interval that was requested.
        evidence: DxbcEvidence,
    },
    /// The requested interval is not currently available.
    Unavailable {
        /// Why the interval is unavailable.
        reason: String,
        /// Interval that was requested.
        evidence: DxbcEvidence,
    },
    /// The source read failed.
    ReadFailed {
        /// Underlying failure text.
        message: String,
        /// Interval that was requested.
        evidence: DxbcEvidence,
    },
    /// The read budget was exhausted before the container was mapped.
    Budget(BudgetError),
}

impl Display for DxbcLayoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownLength => write!(f, "DXBC source does not report a total length"),
            Self::TruncatedHeader { available_bytes, evidence } => {
                write!(f, "DXBC header is truncated at {}: {available_bytes} of 32 bytes available", evidence.location())
            }
            Self::BadMagic { magic, evidence } => {
                write!(f, "DXBC magic mismatch at {}: {magic:02X?}", evidence.location())
            }
            Self::LengthMismatch { declared, actual, evidence } => {
                write!(f, "DXBC declared total length {declared} at {} does not match {actual} bytes", evidence.location())
            }
            Self::ChunkCountOutOfBounds { chunk_count, declared_length, table_bytes, evidence } => {
                write!(
                    f,
                    "DXBC chunk count {chunk_count} at {} needs a {table_bytes}-byte offset table in a {declared_length}-byte container",
                    evidence.location()
                )
            }
            Self::ChunkOffsetOutOfBounds { chunk_index, offset, declared_length, evidence } => {
                write!(
                    f,
                    "DXBC chunk {chunk_index} offset {offset} at {} leaves no chunk header room in a {declared_length}-byte container",
                    evidence.location()
                )
            }
            Self::ChunkSizeOutOfBounds { chunk_index, four_cc, size, offset, declared_length, evidence } => {
                write!(
                    f,
                    "DXBC chunk {chunk_index} ({}) size {size} at offset {offset} exceeds the {declared_length}-byte container and {} at {}",
                    four_cc_text(*four_cc),
                    evidence.byte_range.len,
                    evidence.location()
                )
            }
            Self::ChunkOverlap { first_index, second_index, evidence } => {
                write!(f, "DXBC chunk {first_index} overlaps chunk {second_index} at {}", evidence.location())
            }
            Self::ShaderChunkTooSmall { chunk_index, chunk_size, evidence } => {
                write!(
                    f,
                    "DXBC shader chunk {chunk_index} is {chunk_size} bytes at {}, fewer than the 8-byte token pair",
                    evidence.location()
                )
            }
            Self::ShaderLengthMismatch { chunk_index, declared, chunk_size, evidence } => {
                write!(
                    f,
                    "DXBC shader chunk {chunk_index} length token {declared} DWORDs at {} does not match its {chunk_size}-byte payload",
                    evidence.location()
                )
            }
            Self::ZeroWordCount { evidence } => {
                write!(f, "DXBC instruction at {} declares a zero word count", evidence.location())
            }
            Self::InstructionOutOfBounds { word_count, available_words, evidence } => {
                write!(
                    f,
                    "DXBC instruction at {} declares {word_count} words but only {available_words} remain",
                    evidence.location()
                )
            }
            Self::TrailingPartialWord { remainder_bytes, evidence } => {
                write!(f, "{remainder_bytes} trailing bytes at {} do not complete a 32-bit word", evidence.location())
            }
            Self::ShortRead { expected, actual, evidence } => {
                write!(f, "short read at {}: expected {expected} bytes, read {actual}", evidence.location())
            }
            Self::Unavailable { reason, evidence } => {
                write!(f, "range at {} is unavailable: {reason}", evidence.location())
            }
            Self::ReadFailed { message, evidence } => {
                write!(f, "read at {} failed: {message}", evidence.location())
            }
            Self::Budget(error) => write!(f, "DXBC read budget exhausted: {error}"),
        }
    }
}

impl std::error::Error for DxbcLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            _ => None,
        }
    }
}

fn four_cc_text(four_cc: [u8; 4]) -> String {
    String::from_utf8_lossy(&four_cc).into_owned()
}
