//! Chunk records and the recognized FourCC classes.

use acorn_core::ByteRange;

use crate::header::DXBC_CHUNK_HEADER_BYTES;

/// Kind of a chunk, classified by its FourCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxbcChunkKind {
    /// `RDEF`: resource definitions.
    ResourceDefinition,
    /// `ISGN`: input signature.
    InputSignature,
    /// `OSGN`: output signature.
    OutputSignature,
    /// `SHEX` or `SHDR`: the shader program and its instruction stream.
    ShaderProgram,
    /// `STAT`: statistics.
    Statistics,
    /// Any other FourCC, kept verbatim and skipped by its declared length.
    Unknown([u8; 4]),
}

impl DxbcChunkKind {
    /// Classifies a FourCC as stored in the container.
    pub fn from_four_cc(four_cc: [u8; 4]) -> Self {
        match &four_cc {
            b"RDEF" => Self::ResourceDefinition,
            b"ISGN" => Self::InputSignature,
            b"OSGN" => Self::OutputSignature,
            b"SHEX" | b"SHDR" => Self::ShaderProgram,
            b"STAT" => Self::Statistics,
            _ => Self::Unknown(four_cc),
        }
    }

    /// Reports whether this chunk carries a shader instruction stream.
    pub const fn is_shader(self) -> bool {
        matches!(self, Self::ShaderProgram)
    }
}

/// One chunk: its header fields, its kind and the interval it occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DxbcChunk {
    /// Position of this chunk in the offset table.
    pub index: u32,
    /// FourCC as stored, in file byte order.
    pub four_cc: [u8; 4],
    /// Classification derived from `four_cc`.
    pub kind: DxbcChunkKind,
    /// Absolute offset of the chunk header inside the container.
    pub offset: u32,
    /// Declared payload size in bytes, excluding the 8-byte chunk header.
    pub size: u32,
    /// Interval covering the chunk header and its payload.
    pub byte_range: ByteRange,
}

impl DxbcChunk {
    /// Absolute offset of the chunk payload.
    pub const fn payload_start(&self) -> u64 {
        self.byte_range.start + DXBC_CHUNK_HEADER_BYTES
    }

    /// Exclusive end of the chunk interval.
    pub const fn byte_end(&self) -> u64 {
        self.byte_range.start + self.byte_range.len
    }
}
