//! `SHEX`/`SHDR` tokens, instruction layout and the lazy instruction walk.

use acorn_core::{BudgetTracker, ByteRange};
use acorn_source::ByteSource;

use crate::{
    chunk::DxbcChunk,
    container::read_exact,
    error::{DxbcEvidence, DxbcLayoutError},
    header::{
        DXBC_OPCODE_TOKEN_LENGTH_MASK, DXBC_OPCODE_TOKEN_LENGTH_SHIFT, DXBC_OPCODE_TOKEN_OPCODE_MASK, DXBC_WORD_BYTES,
        SHEX_HEADER_BYTES,
    },
};

/// One instruction in the stream, described only by its layout.
///
/// An SM5 opcode token keeps its length in bits[30:24] and its opcode in
/// bits[10:0], so the token of a one-word instruction only carries flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DxbcInstruction {
    /// Raw opcode from bits[10:0] of the leading word.
    pub opcode: u16,
    /// Declared length in words from bits[30:24], including the leading word.
    pub word_count: u32,
    /// Index of the leading word inside the instruction stream.
    pub word_offset: u64,
    /// Byte range of the whole instruction inside the container.
    pub byte_range: ByteRange,
}

/// Token pair and bounded instruction stream of one `SHEX`/`SHDR` chunk.
///
/// Opening the program reads only the two tokens and bounds the stream. Words
/// are read on demand through [`ShexProgram::instructions`], each settled
/// against the caller's ledger.
#[derive(Debug)]
pub struct ShexProgram<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    chunk_index: u32,
    version_token: u32,
    length_token: u32,
    stream_start: u64,
    stream_words: u64,
}

impl<'a, S: ByteSource + ?Sized> ShexProgram<'a, S> {
    /// Reads the token pair of one shader chunk and bounds its instruction stream.
    ///
    /// The length token counts 32-bit DWORDs, so `length_token * 4` must equal the
    /// byte-sized chunk payload, and the payload after the tokens must be a whole
    /// number of DWORDs. The instructions themselves are not walked here.
    pub fn open(source: &'a S, ledger: &mut BudgetTracker, chunk: &DxbcChunk) -> Result<Self, DxbcLayoutError> {
        let chunk_index = chunk.index;
        let chunk_size = u64::from(chunk.size);
        if chunk_size < SHEX_HEADER_BYTES {
            return Err(DxbcLayoutError::ShaderChunkTooSmall {
                chunk_index,
                chunk_size: chunk.size,
                evidence: DxbcEvidence::chunk(chunk_index, ByteRange { start: chunk.payload_start(), len: chunk_size }),
            });
        }

        let tokens_start = chunk.payload_start();
        let tokens_evidence = DxbcEvidence::chunk(chunk_index, ByteRange { start: tokens_start, len: SHEX_HEADER_BYTES });
        let tokens = read_exact(source, ledger, tokens_start, SHEX_HEADER_BYTES, tokens_evidence)?;
        let version_token = u32::from_le_bytes(tokens[0..4].try_into().expect("4-byte version token"));
        let length_token = u32::from_le_bytes(tokens[4..8].try_into().expect("4-byte length token"));

        let stream_bytes = chunk_size - SHEX_HEADER_BYTES;
        let remainder_bytes = stream_bytes % DXBC_WORD_BYTES;
        if remainder_bytes != 0 {
            let start = tokens_start + SHEX_HEADER_BYTES + (stream_bytes - remainder_bytes);
            return Err(DxbcLayoutError::TrailingPartialWord {
                remainder_bytes,
                evidence: DxbcEvidence::chunk(chunk_index, ByteRange { start, len: remainder_bytes }),
            });
        }

        let declared_bytes = u64::from(length_token) * DXBC_WORD_BYTES;
        if declared_bytes != chunk_size {
            return Err(DxbcLayoutError::ShaderLengthMismatch {
                chunk_index,
                declared: length_token,
                chunk_size: chunk.size,
                evidence: DxbcEvidence::chunk(chunk_index, ByteRange { start: tokens_start + 4, len: 4 }),
            });
        }

        Ok(Self {
            source,
            chunk_index,
            version_token,
            length_token,
            stream_start: tokens_start + SHEX_HEADER_BYTES,
            stream_words: stream_bytes / DXBC_WORD_BYTES,
        })
    }

    /// Index of the chunk this program was opened from.
    pub const fn chunk_index(&self) -> u32 {
        self.chunk_index
    }

    /// Version token, recorded without interpretation.
    pub const fn version_token(&self) -> u32 {
        self.version_token
    }

    /// Length token, in 32-bit DWORDs, covering the whole chunk payload.
    pub const fn length_token(&self) -> u32 {
        self.length_token
    }

    /// Words occupied by the instruction stream, excluding the token pair.
    pub const fn stream_words(&self) -> u64 {
        self.stream_words
    }

    /// Lazily walks the instruction stream.
    ///
    /// Every item is either one instruction's layout or the classified failure
    /// that stopped the walk.
    pub fn instructions<'b>(&self, ledger: &'b mut BudgetTracker) -> ShexInstructions<'a, 'b, S> {
        ShexInstructions {
            source: self.source,
            ledger,
            chunk_index: self.chunk_index,
            stream_start: self.stream_start,
            next_word: 0,
            remaining_words: self.stream_words,
            done: false,
        }
    }
}

/// Lazily walks the instruction stream of one shader chunk.
///
/// Only the leading word of each instruction is read, so the walk is bounded by
/// the instruction count rather than by the chunk size. Every read is settled
/// against the caller's ledger, and the walk stops after the first failure.
#[derive(Debug)]
pub struct ShexInstructions<'a, 'b, S: ByteSource + ?Sized> {
    source: &'a S,
    ledger: &'b mut BudgetTracker,
    chunk_index: u32,
    stream_start: u64,
    next_word: u64,
    remaining_words: u64,
    done: bool,
}

impl<S: ByteSource + ?Sized> Iterator for ShexInstructions<'_, '_, S> {
    type Item = Result<DxbcInstruction, DxbcLayoutError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.remaining_words == 0 {
            return None;
        }

        let word_offset = self.next_word;
        let byte_start = self.stream_start + word_offset * DXBC_WORD_BYTES;
        let evidence = DxbcEvidence::chunk(self.chunk_index, ByteRange { start: byte_start, len: DXBC_WORD_BYTES });
        let bytes = match read_exact(self.source, self.ledger, byte_start, DXBC_WORD_BYTES, evidence) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.done = true;
                return Some(Err(error));
            }
        };
        let word = u32::from_le_bytes(bytes.try_into().expect("one word was read"));
        let word_count = (word >> DXBC_OPCODE_TOKEN_LENGTH_SHIFT) & DXBC_OPCODE_TOKEN_LENGTH_MASK;
        let opcode = (word & DXBC_OPCODE_TOKEN_OPCODE_MASK) as u16;

        if word_count == 0 {
            self.done = true;
            return Some(Err(DxbcLayoutError::ZeroWordCount { evidence }));
        }

        let word_count_wide = u64::from(word_count);
        if word_count_wide > self.remaining_words {
            self.done = true;
            return Some(Err(DxbcLayoutError::InstructionOutOfBounds {
                word_count,
                available_words: self.remaining_words,
                evidence: DxbcEvidence::chunk(
                    self.chunk_index,
                    ByteRange { start: byte_start, len: self.remaining_words * DXBC_WORD_BYTES },
                ),
            }));
        }

        self.next_word += word_count_wide;
        self.remaining_words -= word_count_wide;
        Some(Ok(DxbcInstruction {
            opcode,
            word_count,
            word_offset,
            byte_range: ByteRange { start: byte_start, len: word_count_wide * DXBC_WORD_BYTES },
        }))
    }
}
