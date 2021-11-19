#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

mod bytes;
mod chunk;
mod container;
mod error;
mod header;
mod shex;

/// Byte-slice validation entry and its error surface.
pub use bytes::{DxbcByteError, validate_dxbc_bytes};
/// Chunk records and the recognized FourCC classes.
pub use chunk::{DxbcChunk, DxbcChunkKind};
/// Bounded container view, validation summary and the validation entry.
pub use container::{DxbcContainer, DxbcShaderSummary, DxbcSummary, validate_dxbc};
/// Structural failure classification and its byte evidence.
pub use error::{DxbcEvidence, DxbcLayoutError};
/// Header constants and the recorded container header.
pub use header::{
    DXBC_CHUNK_HEADER_BYTES, DXBC_DIGEST_BYTES, DXBC_HEADER_BYTES, DXBC_MAGIC, DXBC_OFFSET_TABLE_ENTRY_BYTES,
    DXBC_OPCODE_TOKEN_LENGTH_MASK, DXBC_OPCODE_TOKEN_LENGTH_SHIFT, DXBC_OPCODE_TOKEN_OPCODE_MASK, DXBC_WORD_BYTES, DxbcHeader,
    SHEX_HEADER_BYTES,
};
/// `SHEX` tokens, instruction layout and the lazy instruction walk.
pub use shex::{DxbcInstruction, ShexInstructions, ShexProgram};
