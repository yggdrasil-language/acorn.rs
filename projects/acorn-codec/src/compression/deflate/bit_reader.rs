//! LSB-first bit reader for deflate streams.

use crate::bit::BitPosition;
use crate::error::CodecError;

/// Reads bits from a byte slice in deflate LSB-first order.
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buffer: u32,
    bit_count: u8,
}

impl<'a> BitReader<'a> {
    /// Creates a bit reader over `data`.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, byte_pos: 0, bit_buffer: 0, bit_count: 0 }
    }

    /// Current byte offset (including partially consumed bytes).
    pub fn byte_offset(&self) -> usize {
        self.byte_pos
    }

    /// Current bit position inside the stream.
    pub fn position(&self) -> BitPosition {
        BitPosition::new(self.byte_pos as u64, self.bit_count % 8)
    }

    /// Ensures at least `count` bits are available.
    pub fn ensure_bits(&mut self, count: u8) -> Result<(), CodecError> {
        while self.bit_count < count {
            if self.byte_pos >= self.data.len() {
                return Err(CodecError::InputExhausted { offset: self.byte_pos, needed: 1 });
            }
            self.bit_buffer |= (self.data[self.byte_pos] as u32) << self.bit_count;
            self.byte_pos += 1;
            self.bit_count += 8;
        }
        Ok(())
    }

    /// Peeks `count` bits without consuming them (at most 16).
    pub fn peek_bits(&mut self, count: u8) -> Result<u32, CodecError> {
        if count > 16 {
            return Err(CodecError::InvalidBitPattern { offset: self.byte_pos, width: count });
        }
        self.ensure_bits(count)?;
        let mask = if count == 32 { u32::MAX } else { (1u32 << count) - 1 };
        Ok(self.bit_buffer & mask)
    }

    /// Discards `count` bits from the buffer.
    pub fn consume_bits(&mut self, count: u8) -> Result<(), CodecError> {
        if count > self.bit_count {
            return Err(CodecError::InputExhausted { offset: self.byte_pos, needed: 1 });
        }
        self.bit_buffer >>= count;
        self.bit_count -= count;
        Ok(())
    }

    /// Reads `count` bits (at most 16).
    pub fn read_bits(&mut self, count: u8) -> Result<u32, CodecError> {
        if count > 16 {
            return Err(CodecError::InvalidBitPattern { offset: self.byte_pos, width: count });
        }
        self.ensure_bits(count)?;
        let mask = if count == 32 { u32::MAX } else { (1u32 << count) - 1 };
        let value = self.bit_buffer & mask;
        self.bit_buffer >>= count;
        self.bit_count -= count;
        Ok(value)
    }

    /// Reads one byte from the underlying stream (discards partial bits first).
    pub fn read_byte(&mut self) -> Result<u8, CodecError> {
        self.align_to_byte();
        if self.byte_pos >= self.data.len() {
            return Err(CodecError::InputExhausted { offset: self.byte_pos, needed: 1 });
        }
        let byte = self.data[self.byte_pos];
        self.byte_pos += 1;
        Ok(byte)
    }

    /// Reads a little-endian u16 from the underlying stream.
    pub fn read_u16_le(&mut self) -> Result<u16, CodecError> {
        let low = self.read_byte()?;
        let high = self.read_byte()?;
        Ok(u16::from(low) | (u16::from(high) << 8))
    }

    /// Discards bits to align to the next byte boundary.
    pub fn align_to_byte(&mut self) {
        let discard = self.bit_count % 8;
        if discard > 0 {
            self.bit_buffer >>= discard;
            self.bit_count -= discard;
        }
    }

    /// Returns whether all input has been consumed and the bit buffer is empty.
    #[allow(dead_code)]
    pub fn is_exhausted(&self) -> bool {
        self.byte_pos >= self.data.len() && self.bit_count == 0
    }
}
