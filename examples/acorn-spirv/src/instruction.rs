//! Instruction-level layout and the lazy instruction walk.

use acorn_core::{BudgetTracker, ByteRange};
use acorn_source::ByteSource;

use crate::{
    error::{SpirvEvidence, SpirvLayoutError},
    header::{SPIRV_HEADER_WORDS, SPIRV_WORD_BYTES},
    module::read_exact,
};

/// One instruction in the stream, described only by its layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpirvInstruction {
    /// Raw opcode from the low 16 bits of the leading word.
    pub opcode: u16,
    /// Declared length in words, including the leading word.
    pub word_count: u32,
    /// Index of the leading word inside the module.
    pub word_offset: u64,
    /// Byte range of the whole instruction, whether or not its body was read.
    pub byte_range: ByteRange,
}

/// Lazily walks the instruction stream of one module.
///
/// Only the leading word of each instruction is read, so the walk is bounded by
/// the instruction count rather than by the module size. Every read is settled
/// against the caller's ledger, and the walk stops after the first failure.
pub struct SpirvInstructions<'a, 'b, S: ByteSource + ?Sized> {
    source: &'a S,
    ledger: &'b mut BudgetTracker,
    next_word: u64,
    remaining_words: u64,
    done: bool,
}

impl<'a, 'b, S: ByteSource + ?Sized> SpirvInstructions<'a, 'b, S> {
    pub(crate) fn new(source: &'a S, ledger: &'b mut BudgetTracker, instruction_words: u64) -> Self {
        Self { source, ledger, next_word: SPIRV_HEADER_WORDS, remaining_words: instruction_words, done: false }
    }
}

impl<S: ByteSource + ?Sized> Iterator for SpirvInstructions<'_, '_, S> {
    type Item = Result<SpirvInstruction, SpirvLayoutError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.remaining_words == 0 {
            return None;
        }

        let word_offset = self.next_word;
        let byte_start = word_offset * SPIRV_WORD_BYTES;
        let evidence = SpirvEvidence::new(word_offset, ByteRange { start: byte_start, len: SPIRV_WORD_BYTES });
        let bytes = match read_exact(self.source, self.ledger, byte_start, SPIRV_WORD_BYTES, evidence) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.done = true;
                return Some(Err(error));
            }
        };
        let word = u32::from_le_bytes(bytes.try_into().expect("one word was read"));
        let word_count = word >> 16;
        let opcode = (word & 0xFFFF) as u16;

        if word_count == 0 {
            self.done = true;
            return Some(Err(SpirvLayoutError::ZeroWordCount { evidence }));
        }

        let word_count_wide = u64::from(word_count);
        if word_count_wide > self.remaining_words {
            self.done = true;
            return Some(Err(SpirvLayoutError::InstructionOutOfBounds {
                word_count,
                available_words: self.remaining_words,
                evidence: SpirvEvidence::new(
                    word_offset,
                    ByteRange { start: byte_start, len: self.remaining_words * SPIRV_WORD_BYTES },
                ),
            }));
        }

        self.next_word += word_count_wide;
        self.remaining_words -= word_count_wide;
        Some(Ok(SpirvInstruction {
            opcode,
            word_count,
            word_offset,
            byte_range: ByteRange { start: byte_start, len: word_count_wide * SPIRV_WORD_BYTES },
        }))
    }
}
