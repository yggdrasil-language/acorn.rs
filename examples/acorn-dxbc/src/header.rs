//! Container header constants and the recorded header words.

/// Magic bytes that start every DXBC container.
pub const DXBC_MAGIC: [u8; 4] = *b"DXBC";
/// Size of the container digest in bytes.
pub const DXBC_DIGEST_BYTES: u64 = 16;
/// Size of the container header in bytes, up to and including the chunk count.
pub const DXBC_HEADER_BYTES: u64 = 32;
/// Size of one chunk offset table entry in bytes.
pub const DXBC_OFFSET_TABLE_ENTRY_BYTES: u64 = 4;
/// Size of a chunk header in bytes: the FourCC plus the declared payload size.
pub const DXBC_CHUNK_HEADER_BYTES: u64 = 8;
/// Size of one instruction word in bytes.
pub const DXBC_WORD_BYTES: u64 = 4;
/// Shift of the instruction length field inside an SM5 opcode token.
pub const DXBC_OPCODE_TOKEN_LENGTH_SHIFT: u32 = 24;
/// Mask of the instruction length field inside an SM5 opcode token.
///
/// The length counts DWORDs including the opcode token itself and occupies
/// bits[30:24], so bit 31 stays reserved.
pub const DXBC_OPCODE_TOKEN_LENGTH_MASK: u32 = 0x0000_007F;
/// Mask of the opcode field inside an SM5 opcode token, in bits[10:0].
pub const DXBC_OPCODE_TOKEN_OPCODE_MASK: u32 = 0x0000_07FF;
/// Size of the `SHEX`/`SHDR` version and length token pair in bytes.
pub const SHEX_HEADER_BYTES: u64 = 8;

/// The container header, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DxbcHeader {
    /// Magic bytes, always [`DXBC_MAGIC`] for a structurally valid container.
    pub magic: [u8; 4],
    /// Sixteen-byte container digest, recorded without verification.
    pub digest: [u8; DXBC_DIGEST_BYTES as usize],
    /// Container version word, recorded without interpretation.
    pub version: u32,
    /// Total container length in bytes, as declared by the header.
    pub total_length: u32,
    /// Number of chunk offsets that follow the header.
    pub chunk_count: u32,
}

impl DxbcHeader {
    /// Bytes occupied by the chunk offset table this header declares.
    pub const fn offset_table_bytes(&self) -> u64 {
        self.chunk_count as u64 * DXBC_OFFSET_TABLE_ENTRY_BYTES
    }
}
