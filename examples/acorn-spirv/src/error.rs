//! Structural failures and the byte evidence they keep.

use std::fmt::{Display, Formatter};

use acorn_core::{BudgetError, ByteRange};

/// Byte and word coordinates for one structural claim.
///
/// `byte_range` is relative to the start of the module and `word_offset` counts
/// 32-bit words from that same start, so both coordinates describe one interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpirvEvidence {
    /// Index of the first 32-bit word the interval touches.
    pub word_offset: u64,
    /// Byte interval inside the module.
    pub byte_range: ByteRange,
}

impl SpirvEvidence {
    /// Creates evidence for an interval that starts at `byte_range.start`.
    pub const fn new(word_offset: u64, byte_range: ByteRange) -> Self {
        Self { word_offset, byte_range }
    }

    /// Number of words the interval covers, rounded up for a partial trailing word.
    pub fn word_count(&self) -> u64 {
        self.byte_range.len.div_ceil(4)
    }
}

/// Structural failure found while mapping a SPIR-V module.
///
/// Each variant keeps the coordinates of the bytes that produced it, so a
/// consumer can point at the module instead of re-deriving the location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpirvLayoutError {
    /// The source does not report a total length.
    UnknownLength,
    /// Fewer than five header words are available.
    TruncatedHeader {
        /// Bytes the source reports before the header is complete.
        available_bytes: u64,
        /// Interval that should have held the header.
        evidence: SpirvEvidence,
    },
    /// The first word is not the SPIR-V magic.
    BadMagic {
        /// Magic word that was read.
        magic: u32,
        /// Interval of the magic word.
        evidence: SpirvEvidence,
    },
    /// The reserved schema word is not zero.
    NonZeroSchema {
        /// Schema word that was read.
        schema: u32,
        /// Interval of the schema word.
        evidence: SpirvEvidence,
    },
    /// The total byte length does not end on a 32-bit word boundary.
    TrailingPartialWord {
        /// Bytes left after the last complete word.
        remainder_bytes: u64,
        /// Interval of the trailing bytes.
        evidence: SpirvEvidence,
    },
    /// The header is followed by no instruction words.
    EmptyInstructionStream {
        /// Empty interval where the first instruction word was expected.
        evidence: SpirvEvidence,
    },
    /// An instruction declares a zero word count.
    ZeroWordCount {
        /// Interval of the instruction's leading word.
        evidence: SpirvEvidence,
    },
    /// An instruction's declared word count runs past the end of the module.
    InstructionOutOfBounds {
        /// Word count declared by the instruction.
        word_count: u32,
        /// Words that remain in the stream.
        available_words: u64,
        /// Interval of the remaining instruction words.
        evidence: SpirvEvidence,
    },
    /// The source returned fewer bytes than requested.
    ShortRead {
        /// Requested byte count.
        expected: u64,
        /// Bytes the source returned.
        actual: u64,
        /// Interval that was requested.
        evidence: SpirvEvidence,
    },
    /// The requested interval is not currently available.
    Unavailable {
        /// Why the interval is unavailable.
        reason: String,
        /// Interval that was requested.
        evidence: SpirvEvidence,
    },
    /// The source read failed.
    ReadFailed {
        /// Underlying failure text.
        message: String,
        /// Interval that was requested.
        evidence: SpirvEvidence,
    },
    /// The read budget was exhausted before the module was mapped.
    Budget(BudgetError),
}

impl Display for SpirvLayoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownLength => write!(f, "SPIR-V source does not report a total length"),
            Self::TruncatedHeader { available_bytes, evidence } => {
                write!(
                    f,
                    "SPIR-V header is truncated: {available_bytes} of 20 bytes available at word {}",
                    evidence.word_offset
                )
            }
            Self::BadMagic { magic, evidence } => {
                write!(f, "SPIR-V magic mismatch at word {}: {magic:#010X}", evidence.word_offset)
            }
            Self::NonZeroSchema { schema, evidence } => {
                write!(f, "SPIR-V schema word at word {} must be zero, found {schema}", evidence.word_offset)
            }
            Self::TrailingPartialWord { remainder_bytes, evidence } => {
                write!(f, "{remainder_bytes} trailing bytes at word {} do not complete a 32-bit word", evidence.word_offset)
            }
            Self::EmptyInstructionStream { evidence } => {
                write!(f, "SPIR-V header at word {} is followed by no instruction words", evidence.word_offset)
            }
            Self::ZeroWordCount { evidence } => {
                write!(f, "SPIR-V instruction at word {} declares a zero word count", evidence.word_offset)
            }
            Self::InstructionOutOfBounds { word_count, available_words, evidence } => {
                write!(
                    f,
                    "SPIR-V instruction at word {} declares {word_count} words but only {available_words} remain",
                    evidence.word_offset
                )
            }
            Self::ShortRead { expected, actual, evidence } => {
                write!(f, "short read at word {}: expected {expected} bytes, read {actual}", evidence.word_offset)
            }
            Self::Unavailable { reason, evidence } => {
                write!(f, "range at word {} is unavailable: {reason}", evidence.word_offset)
            }
            Self::ReadFailed { message, evidence } => {
                write!(f, "read at word {} failed: {message}", evidence.word_offset)
            }
            Self::Budget(error) => write!(f, "SPIR-V read budget exhausted: {error}"),
        }
    }
}

impl std::error::Error for SpirvLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            _ => None,
        }
    }
}
