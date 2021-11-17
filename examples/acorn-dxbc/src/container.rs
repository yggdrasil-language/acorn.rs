//! Bounded container view and the structural validation entry.

use acorn_core::{BudgetTracker, ByteRange};
use acorn_source::{ByteSource, ReadError, SourceRange};

use crate::{
    chunk::{DxbcChunk, DxbcChunkKind},
    error::{DxbcEvidence, DxbcLayoutError},
    header::{DXBC_CHUNK_HEADER_BYTES, DXBC_HEADER_BYTES, DXBC_MAGIC, DXBC_OFFSET_TABLE_ENTRY_BYTES, DxbcHeader},
    shex::ShexProgram,
};

/// What a structurally valid container declares about its own layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxbcSummary {
    /// Header words that were read.
    pub header: DxbcHeader,
    /// Chunks in offset table order.
    pub chunks: Vec<DxbcChunk>,
    /// Token pair and instruction counts, one entry per shader chunk.
    pub shaders: Vec<DxbcShaderSummary>,
}

/// `SHEX`/`SHDR` tokens and the instruction counts recorded for one shader chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DxbcShaderSummary {
    /// Chunk that carries the instruction stream.
    pub chunk_index: u32,
    /// Version token recorded by the shader chunk.
    pub version_token: u32,
    /// Length token recorded by the shader chunk.
    pub length_token: u32,
    /// Number of instructions in the stream.
    pub instruction_count: u64,
    /// Words occupied by instructions, excluding the token pair.
    pub instruction_words: u64,
}

/// Bounded view over one DXBC container: header, offset table and chunks.
///
/// Opening the container reads only the fixed header and the offset table.
/// Chunk headers, shader tokens and instruction words are read on demand through
/// [`DxbcContainer::chunks`] and [`DxbcContainer::validate`], each settled
/// against the caller's ledger.
#[derive(Debug)]
pub struct DxbcContainer<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    header: DxbcHeader,
    total_length: u64,
    chunk_offsets: Vec<u32>,
}

impl<'a, S: ByteSource + ?Sized> DxbcContainer<'a, S> {
    /// Maps the container header and offset table.
    ///
    /// The magic and the declared total length are checked here, together with
    /// the room the offset table needs. Chunk headers are not read, so a
    /// chunk-level failure is reported by [`DxbcContainer::chunks`] with its own
    /// evidence.
    pub fn open(source: &'a S, ledger: &mut BudgetTracker) -> Result<Self, DxbcLayoutError> {
        let length = source.length().ok_or(DxbcLayoutError::UnknownLength)?;
        if length < DXBC_HEADER_BYTES {
            return Err(DxbcLayoutError::TruncatedHeader {
                available_bytes: length,
                evidence: DxbcEvidence::container(ByteRange { start: 0, len: length }),
            });
        }

        let header_bytes = read_exact(
            source,
            ledger,
            0,
            DXBC_HEADER_BYTES,
            DxbcEvidence::container(ByteRange { start: 0, len: DXBC_HEADER_BYTES }),
        )?;
        let magic: [u8; 4] = header_bytes[0..4].try_into().expect("4-byte magic");
        if magic != DXBC_MAGIC {
            return Err(DxbcLayoutError::BadMagic { magic, evidence: DxbcEvidence::container(ByteRange { start: 0, len: 4 }) });
        }

        let mut digest = [0u8; 16];
        digest.copy_from_slice(&header_bytes[4..20]);
        let version = u32::from_le_bytes(header_bytes[20..24].try_into().expect("4-byte version word"));
        let total_length = u32::from_le_bytes(header_bytes[24..28].try_into().expect("4-byte length word"));
        let chunk_count = u32::from_le_bytes(header_bytes[28..32].try_into().expect("4-byte chunk count"));

        if u64::from(total_length) != length {
            return Err(DxbcLayoutError::LengthMismatch {
                declared: total_length,
                actual: length,
                evidence: DxbcEvidence::container(ByteRange { start: 24, len: 4 }),
            });
        }

        let header = DxbcHeader { magic, digest, version, total_length, chunk_count };
        let table_bytes = header.offset_table_bytes();
        if DXBC_HEADER_BYTES + table_bytes > length {
            return Err(DxbcLayoutError::ChunkCountOutOfBounds {
                chunk_count,
                declared_length: length,
                table_bytes,
                evidence: DxbcEvidence::container(ByteRange { start: 28, len: 4 }),
            });
        }

        let chunk_offsets = if chunk_count == 0 {
            Vec::new()
        }
        else {
            let table = read_exact(
                source,
                ledger,
                DXBC_HEADER_BYTES,
                table_bytes,
                DxbcEvidence::container(ByteRange { start: DXBC_HEADER_BYTES, len: table_bytes }),
            )?;
            let mut chunk_offsets = Vec::with_capacity(chunk_count as usize);
            for entry in table.chunks(DXBC_OFFSET_TABLE_ENTRY_BYTES as usize) {
                chunk_offsets.push(u32::from_le_bytes(entry.try_into().expect("4-byte offset")));
            }
            chunk_offsets
        };

        Ok(Self { source, header, total_length: length, chunk_offsets })
    }

    /// Header words read when the container was opened.
    pub const fn header(&self) -> DxbcHeader {
        self.header
    }

    /// Chunk offsets in table order, as declared by the header.
    pub fn chunk_offsets(&self) -> &[u32] {
        &self.chunk_offsets
    }

    /// Reads every chunk header and checks the chunk geometry.
    ///
    /// Each chunk's FourCC and declared size are recorded, its interval is
    /// bounded by the container length, and no two chunks may overlap.
    pub fn chunks(&self, ledger: &mut BudgetTracker) -> Result<Vec<DxbcChunk>, DxbcLayoutError> {
        let mut chunks = Vec::with_capacity(self.chunk_offsets.len());
        for (position, offset) in self.chunk_offsets.iter().copied().enumerate() {
            let index = position as u32;
            let offset_wide = u64::from(offset);
            if offset_wide + DXBC_CHUNK_HEADER_BYTES > self.total_length {
                return Err(DxbcLayoutError::ChunkOffsetOutOfBounds {
                    chunk_index: index,
                    offset,
                    declared_length: self.total_length,
                    evidence: DxbcEvidence::chunk(index, ByteRange { start: offset_wide, len: DXBC_CHUNK_HEADER_BYTES }),
                });
            }

            let header = read_exact(
                self.source,
                ledger,
                offset_wide,
                DXBC_CHUNK_HEADER_BYTES,
                DxbcEvidence::chunk(index, ByteRange { start: offset_wide, len: DXBC_CHUNK_HEADER_BYTES }),
            )?;
            let four_cc: [u8; 4] = header[0..4].try_into().expect("4-byte FourCC");
            let size = u32::from_le_bytes(header[4..8].try_into().expect("4-byte chunk size"));
            let payload_len = u64::from(size);

            if offset_wide + DXBC_CHUNK_HEADER_BYTES + payload_len > self.total_length {
                return Err(DxbcLayoutError::ChunkSizeOutOfBounds {
                    chunk_index: index,
                    four_cc,
                    size,
                    offset,
                    declared_length: self.total_length,
                    evidence: DxbcEvidence::chunk(
                        index,
                        ByteRange { start: offset_wide + DXBC_CHUNK_HEADER_BYTES, len: payload_len },
                    ),
                });
            }

            chunks.push(DxbcChunk {
                index,
                four_cc,
                kind: DxbcChunkKind::from_four_cc(four_cc),
                offset,
                size,
                byte_range: ByteRange { start: offset_wide, len: DXBC_CHUNK_HEADER_BYTES + payload_len },
            });
        }

        self.check_chunk_overlap(&chunks)?;
        Ok(chunks)
    }

    /// Validates every chunk, including each shader instruction stream.
    pub fn validate(&self, ledger: &mut BudgetTracker) -> Result<DxbcSummary, DxbcLayoutError> {
        let chunks = self.chunks(ledger)?;
        let mut shaders = Vec::new();
        for chunk in &chunks {
            if !chunk.kind.is_shader() {
                continue;
            }

            let program = ShexProgram::open(self.source, ledger, chunk)?;
            let mut instruction_count = 0u64;
            let mut instruction_words = 0u64;
            for item in program.instructions(ledger) {
                let instruction = item?;
                instruction_count += 1;
                instruction_words += u64::from(instruction.word_count);
            }
            shaders.push(DxbcShaderSummary {
                chunk_index: chunk.index,
                version_token: program.version_token(),
                length_token: program.length_token(),
                instruction_count,
                instruction_words,
            });
        }

        Ok(DxbcSummary { header: self.header, chunks, shaders })
    }

    fn check_chunk_overlap(&self, chunks: &[DxbcChunk]) -> Result<(), DxbcLayoutError> {
        let mut order: Vec<usize> = (0..chunks.len()).collect();
        order.sort_by_key(|position| chunks[*position].offset);
        for pair in order.windows(2) {
            let first = &chunks[pair[0]];
            let second = &chunks[pair[1]];
            if first.byte_end() > u64::from(second.offset) {
                return Err(DxbcLayoutError::ChunkOverlap {
                    first_index: first.index,
                    second_index: second.index,
                    evidence: DxbcEvidence::chunk(second.index, second.byte_range),
                });
            }
        }
        Ok(())
    }
}

/// Maps and validates a DXBC container in one step.
///
/// Returns the container summary for a structurally valid container, or the
/// classified failure that was found. Chunk semantics and instruction meaning
/// are never inspected.
pub fn validate_dxbc<S: ByteSource + ?Sized>(source: &S, ledger: &mut BudgetTracker) -> Result<DxbcSummary, DxbcLayoutError> {
    DxbcContainer::open(source, ledger)?.validate(ledger)
}

/// Reads exactly `len` bytes and settles the read against `ledger`.
pub(crate) fn read_exact<S: ByteSource + ?Sized>(
    source: &S,
    ledger: &mut BudgetTracker,
    start: u64,
    len: u64,
    evidence: DxbcEvidence,
) -> Result<Vec<u8>, DxbcLayoutError> {
    ledger.reserve_read_bytes(len).map_err(DxbcLayoutError::Budget)?;
    match source.read_range(SourceRange { start, len }) {
        Ok(bytes) => {
            let consumed = bytes.len() as u64;
            ledger.settle_read_bytes(consumed).map_err(DxbcLayoutError::Budget)?;
            if consumed < len {
                ledger.release_read_bytes(len - consumed).map_err(DxbcLayoutError::Budget)?;
                return Err(DxbcLayoutError::ShortRead { expected: len, actual: consumed, evidence });
            }
            Ok(bytes)
        }
        Err(ReadError::ShortRead { expected, actual }) => {
            let consumed = actual.min(len);
            ledger.settle_read_bytes(consumed).map_err(DxbcLayoutError::Budget)?;
            if consumed < len {
                ledger.release_read_bytes(len - consumed).map_err(DxbcLayoutError::Budget)?;
            }
            Err(DxbcLayoutError::ShortRead { expected, actual, evidence })
        }
        Err(error) => {
            ledger.release_read_bytes(len).map_err(DxbcLayoutError::Budget)?;
            Err(match error {
                ReadError::Unavailable { reason, .. } => DxbcLayoutError::Unavailable { reason, evidence },
                other => DxbcLayoutError::ReadFailed { message: other.to_string(), evidence },
            })
        }
    }
}
