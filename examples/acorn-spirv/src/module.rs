//! Bounded module view and the structural validation entry.

use acorn_core::{BudgetTracker, ByteRange};
use acorn_source::{ByteSource, ReadError, SourceRange};

use crate::{
    error::{SpirvEvidence, SpirvLayoutError},
    header::{SPIRV_HEADER_BYTES, SPIRV_HEADER_WORDS, SPIRV_MAGIC, SPIRV_WORD_BYTES, SpirvBoundSuspicion, SpirvHeader},
    instruction::SpirvInstructions,
};

/// What a structurally valid module declares about its own layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpirvSummary {
    /// Header words that were read.
    pub header: SpirvHeader,
    /// Number of instructions in the stream.
    pub instruction_count: u64,
    /// Words occupied by instructions, excluding the header.
    pub instruction_words: u64,
    /// Words occupied by the whole module, including the header.
    pub total_words: u64,
    /// Recorded sanity observation on the declared id bound.
    pub bound_suspicion: Option<SpirvBoundSuspicion>,
}

/// Bounded view over one SPIR-V module: header plus an instruction stream.
///
/// Opening the module reads only the header and checks the stream geometry.
/// Instruction words are read on demand through [`SpirvModule::instructions`] or
/// [`SpirvModule::validate`], each settled against the caller's ledger.
#[derive(Debug)]
pub struct SpirvModule<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    header: SpirvHeader,
    instruction_words: u64,
}

impl<'a, S: ByteSource + ?Sized> SpirvModule<'a, S> {
    /// Maps the module header and bounds the instruction stream.
    ///
    /// The header is read once and checked for magic, a zero schema word and a
    /// word-aligned total length. The instruction stream itself is not walked
    /// here, so an instruction-level failure is reported by
    /// [`SpirvModule::validate`] with its own evidence.
    pub fn open(source: &'a S, ledger: &mut BudgetTracker) -> Result<Self, SpirvLayoutError> {
        let length = source.length().ok_or(SpirvLayoutError::UnknownLength)?;
        if length < SPIRV_HEADER_BYTES {
            return Err(SpirvLayoutError::TruncatedHeader {
                available_bytes: length,
                evidence: SpirvEvidence::new(0, ByteRange { start: 0, len: length }),
            });
        }

        let header_bytes = read_exact(
            source,
            ledger,
            0,
            SPIRV_HEADER_BYTES,
            SpirvEvidence::new(0, ByteRange { start: 0, len: SPIRV_HEADER_BYTES }),
        )?;
        let word_at =
            |index: usize| u32::from_le_bytes(header_bytes[index * 4..index * 4 + 4].try_into().expect("4-byte header word"));
        let magic = word_at(0);
        let version = word_at(1);
        let generator = word_at(2);
        let bound = word_at(3);
        let schema = word_at(4);

        if magic != SPIRV_MAGIC {
            return Err(SpirvLayoutError::BadMagic {
                magic,
                evidence: SpirvEvidence::new(0, ByteRange { start: 0, len: SPIRV_WORD_BYTES }),
            });
        }
        if schema != 0 {
            return Err(SpirvLayoutError::NonZeroSchema {
                schema,
                evidence: SpirvEvidence::new(4, ByteRange { start: 4 * SPIRV_WORD_BYTES, len: SPIRV_WORD_BYTES }),
            });
        }

        let remainder = length % SPIRV_WORD_BYTES;
        if remainder != 0 {
            let start = length - remainder;
            return Err(SpirvLayoutError::TrailingPartialWord {
                remainder_bytes: remainder,
                evidence: SpirvEvidence::new(start / SPIRV_WORD_BYTES, ByteRange { start, len: remainder }),
            });
        }

        let instruction_words = (length - SPIRV_HEADER_BYTES) / SPIRV_WORD_BYTES;
        if instruction_words == 0 {
            return Err(SpirvLayoutError::EmptyInstructionStream {
                evidence: SpirvEvidence::new(SPIRV_HEADER_WORDS, ByteRange { start: SPIRV_HEADER_BYTES, len: 0 }),
            });
        }

        Ok(Self { source, header: SpirvHeader { magic, version, generator, bound, schema }, instruction_words })
    }

    /// Header words read when the module was opened.
    pub const fn header(&self) -> SpirvHeader {
        self.header
    }

    /// Words occupied by instructions, excluding the header.
    pub const fn instruction_words(&self) -> u64 {
        self.instruction_words
    }

    /// Words occupied by the whole module, including the header.
    pub const fn total_words(&self) -> u64 {
        SPIRV_HEADER_WORDS + self.instruction_words
    }

    /// Lazily walks the instruction stream.
    ///
    /// Every item is either one instruction's layout or the classified failure
    /// that stopped the walk.
    pub fn instructions<'b>(&self, ledger: &'b mut BudgetTracker) -> SpirvInstructions<'a, 'b, S> {
        SpirvInstructions::new(self.source, ledger, self.instruction_words)
    }

    /// Walks the whole instruction stream and summarizes it.
    pub fn validate(&self, ledger: &mut BudgetTracker) -> Result<SpirvSummary, SpirvLayoutError> {
        let mut instruction_count = 0u64;
        let mut instruction_words = 0u64;
        for item in self.instructions(ledger) {
            let instruction = item?;
            instruction_count += 1;
            instruction_words += u64::from(instruction.word_count);
        }

        Ok(SpirvSummary {
            header: self.header,
            instruction_count,
            instruction_words,
            total_words: SPIRV_HEADER_WORDS + instruction_words,
            bound_suspicion: self.header.bound_suspicion(),
        })
    }
}

/// Maps and validates a SPIR-V module in one step.
///
/// Returns the module summary for a structurally valid module, or the classified
/// failure that was found. Instruction semantics are never inspected.
pub fn validate_spirv<S: ByteSource + ?Sized>(
    source: &S,
    ledger: &mut BudgetTracker,
) -> Result<SpirvSummary, SpirvLayoutError> {
    SpirvModule::open(source, ledger)?.validate(ledger)
}

/// Reads exactly `len` bytes and settles the read against `ledger`.
pub(crate) fn read_exact<S: ByteSource + ?Sized>(
    source: &S,
    ledger: &mut BudgetTracker,
    start: u64,
    len: u64,
    evidence: SpirvEvidence,
) -> Result<Vec<u8>, SpirvLayoutError> {
    ledger.reserve_read_bytes(len).map_err(SpirvLayoutError::Budget)?;
    match source.read_range(SourceRange { start, len }) {
        Ok(bytes) => {
            let consumed = bytes.len() as u64;
            ledger.settle_read_bytes(consumed).map_err(SpirvLayoutError::Budget)?;
            if consumed < len {
                ledger.release_read_bytes(len - consumed).map_err(SpirvLayoutError::Budget)?;
                return Err(SpirvLayoutError::ShortRead { expected: len, actual: consumed, evidence });
            }
            Ok(bytes)
        }
        Err(ReadError::ShortRead { expected, actual }) => {
            let consumed = actual.min(len);
            ledger.settle_read_bytes(consumed).map_err(SpirvLayoutError::Budget)?;
            if consumed < len {
                ledger.release_read_bytes(len - consumed).map_err(SpirvLayoutError::Budget)?;
            }
            Err(SpirvLayoutError::ShortRead { expected, actual, evidence })
        }
        Err(error) => {
            ledger.release_read_bytes(len).map_err(SpirvLayoutError::Budget)?;
            Err(match error {
                ReadError::Unavailable { reason, .. } => SpirvLayoutError::Unavailable { reason, evidence },
                other => SpirvLayoutError::ReadFailed { message: other.to_string(), evidence },
            })
        }
    }
}
