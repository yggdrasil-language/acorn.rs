//! Bit-level position inside a codec buffer.

use std::fmt;

/// Position of a bit inside a codec input or output buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct BitPosition {
    /// Byte offset counted from the start of the buffer.
    pub byte_offset: u64,
    /// Bit offset inside `byte_offset`, counted in stream order.
    pub bit_offset: u8,
}

impl BitPosition {
    /// Creates a bit position from a byte offset and a bit offset.
    #[inline]
    pub const fn new(byte_offset: u64, bit_offset: u8) -> Self {
        Self { byte_offset, bit_offset }
    }

    /// Total number of bits counted from the start of the buffer.
    #[inline]
    pub const fn total_bits(&self) -> u64 {
        self.byte_offset * 8 + self.bit_offset as u64
    }
}

impl fmt::Display for BitPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "byte {} bit {}", self.byte_offset, self.bit_offset)
    }
}
