//! Bit-level input over a finite byte slice.

use crate::{
    bit::{BitOrder, BitPosition},
    error::CodecError,
};

/// Maximum number of bits a single read may request.
pub const MAX_READ_BITS: u8 = 32;

/// Bit cursor over an immutable byte slice.
///
/// The cursor tracks the logical position only. Prefetched whole bytes stay in
/// the internal buffer, so [`BitReader::position`] and [`BitReader::read_byte`]
/// always agree on which byte comes next.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    order: BitOrder,
    byte_pos: usize,
    bit_buffer: u64,
    bit_count: u8,
}

impl<'a> BitReader<'a> {
    /// Creates a reader over `data` using `order`.
    pub const fn new(data: &'a [u8], order: BitOrder) -> Self {
        Self { data, order, byte_pos: 0, bit_buffer: 0, bit_count: 0 }
    }

    /// Returns the configured bit order.
    #[inline]
    pub const fn order(&self) -> BitOrder {
        self.order
    }

    /// Returns the position of the next bit to read.
    pub fn position(&self) -> BitPosition {
        let loaded_bits = self.byte_pos as u64 * 8;
        let consumed_bits = loaded_bits - u64::from(self.bit_count);
        BitPosition::new(consumed_bits / 8, (consumed_bits % 8) as u8)
    }

    /// Returns the byte offset of the next bit to read.
    #[inline]
    pub fn byte_offset(&self) -> usize {
        self.position().byte_offset as usize
    }

    /// Number of bytes that contain consumed bits.
    ///
    /// A Deflate stream may end mid-byte, so callers that need the position of a
    /// following trailer round up to the last byte that was touched.
    #[inline]
    pub fn consumed_bytes(&self) -> usize {
        self.position().total_bits().div_ceil(8) as usize
    }

    /// Returns the number of bits that are still readable.
    #[inline]
    pub fn remaining_bits(&self) -> u64 {
        self.data.len() as u64 * 8 - self.position().total_bits()
    }

    /// Whether the cursor sits on a byte boundary.
    #[inline]
    pub fn is_aligned(&self) -> bool {
        self.bit_count % 8 == 0
    }

    /// Reads one bit.
    #[inline]
    pub fn read_bit(&mut self) -> Result<u32, CodecError> {
        self.read_bits(1)
    }

    /// Reads `count` bits without consuming them.
    pub fn peek_bits(&mut self, count: u8) -> Result<u32, CodecError> {
        if count > MAX_READ_BITS {
            return Err(CodecError::InvalidBitPattern { offset: self.byte_offset(), width: count });
        }
        self.ensure_bits(count)?;
        Ok(self.extract(count))
    }

    /// Reads `count` bits and advances the cursor.
    pub fn read_bits(&mut self, count: u8) -> Result<u32, CodecError> {
        let value = self.peek_bits(count)?;
        self.consume_bits(count)?;
        Ok(value)
    }

    /// Discards `count` bits without reading their value.
    pub fn consume_bits(&mut self, count: u8) -> Result<(), CodecError> {
        if count > MAX_READ_BITS {
            return Err(CodecError::InvalidBitPattern { offset: self.byte_offset(), width: count });
        }
        self.ensure_bits(count)?;
        self.bit_buffer >>= count;
        self.bit_count -= count;
        Ok(())
    }

    /// Discards the remaining bits of the current byte.
    ///
    /// Prefetched whole bytes stay buffered, so the next read continues at the
    /// byte boundary without skipping bytes.
    pub fn align_to_byte(&mut self) {
        let discard = self.bit_count % 8;
        self.bit_buffer >>= discard;
        self.bit_count -= discard;
    }

    /// Reads a whole byte, discarding the remaining bits of the current byte.
    pub fn read_byte(&mut self) -> Result<u8, CodecError> {
        self.align_to_byte();
        Ok(self.read_bits(8)? as u8)
    }

    /// Reads a little-endian `u16` from two whole bytes.
    pub fn read_u16_le(&mut self) -> Result<u16, CodecError> {
        let low = self.read_byte()?;
        let high = self.read_byte()?;
        Ok(u16::from(low) | (u16::from(high) << 8))
    }

    fn ensure_bits(&mut self, count: u8) -> Result<(), CodecError> {
        if self.bit_count >= count {
            return Ok(());
        }
        let missing = count - self.bit_count;
        let bytes = usize::from(missing.div_ceil(8));
        if self.byte_pos + bytes > self.data.len() {
            return Err(CodecError::InputExhausted { offset: self.byte_offset(), needed: bytes });
        }
        for _ in 0..bytes {
            let bits = match self.order {
                BitOrder::LeastSignificantBitFirst => u64::from(self.data[self.byte_pos]),
                BitOrder::MostSignificantBitFirst => u64::from(self.data[self.byte_pos].reverse_bits()),
            };
            self.bit_buffer |= bits << self.bit_count;
            self.byte_pos += 1;
            self.bit_count += 8;
        }
        Ok(())
    }

    fn extract(&self, count: u8) -> u32 {
        if count == 0 {
            return 0;
        }
        let mask = (1u64 << count) - 1;
        let raw = (self.bit_buffer & mask) as u32;
        match self.order {
            BitOrder::LeastSignificantBitFirst => raw,
            BitOrder::MostSignificantBitFirst => raw.reverse_bits() >> (32 - count),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bit::{BitOrder, BitPosition, BitReader},
        error::CodecError,
    };

    #[test]
    fn least_significant_bit_first_reads_field_in_stream_order() {
        let mut reader = BitReader::new(&[0b1011_0110, 0b0000_0011], BitOrder::LeastSignificantBitFirst);
        assert_eq!(reader.read_bits(3).expect("low bits"), 0b110);
        assert_eq!(reader.position(), BitPosition::new(0, 3));
        // Crossing the byte boundary keeps stream order.
        assert_eq!(reader.read_bits(6).expect("spanning bits"), 0b110_110);
        assert_eq!(reader.position(), BitPosition::new(1, 1));
    }

    #[test]
    fn most_significant_bit_first_reads_field_value() {
        let mut reader = BitReader::new(&[0b1011_0110], BitOrder::MostSignificantBitFirst);
        assert_eq!(reader.read_bits(4).expect("high nibble"), 0b1011);
        assert_eq!(reader.read_bits(4).expect("low nibble"), 0b0110);
        assert_eq!(reader.position(), BitPosition::new(1, 0));
    }

    #[test]
    fn whole_byte_reads_match_the_declared_order() {
        let mut least = BitReader::new(&[0b1011_0110], BitOrder::LeastSignificantBitFirst);
        assert_eq!(least.read_byte().expect("byte"), 0b1011_0110);
        let mut most = BitReader::new(&[0b1011_0110], BitOrder::MostSignificantBitFirst);
        assert_eq!(most.read_byte().expect("byte"), 0b1011_0110);
    }

    #[test]
    fn peek_and_align_do_not_skip_buffered_bytes() {
        let mut reader = BitReader::new(&[0xAA, 0xBB], BitOrder::LeastSignificantBitFirst);
        assert_eq!(reader.peek_bits(12).expect("peek"), 0x0BAA);
        assert_eq!(reader.position(), BitPosition::new(0, 0));
        reader.align_to_byte();
        assert_eq!(reader.read_byte().expect("first byte"), 0xAA);
        assert_eq!(reader.read_byte().expect("second byte"), 0xBB);
        assert_eq!(reader.byte_offset(), 2);
    }

    #[test]
    fn consumed_bytes_round_up_to_the_touched_byte() {
        let mut reader = BitReader::new(&[0x0F, 0x0F], BitOrder::LeastSignificantBitFirst);
        assert_eq!(reader.read_bits(4).expect("half byte"), 0x0F);
        assert_eq!(reader.byte_offset(), 0);
        assert_eq!(reader.consumed_bytes(), 1);
        assert_eq!(reader.read_bits(8).expect("crossing byte"), 0xF0);
        assert_eq!(reader.consumed_bytes(), 2);
    }

    #[test]
    fn truncated_input_is_reported_with_position() {
        let mut reader = BitReader::new(&[0x0F], BitOrder::LeastSignificantBitFirst);
        assert_eq!(reader.read_bits(4).expect("half byte"), 0x0F);
        let error = reader.read_bits(9).expect_err("truncated");
        assert!(matches!(error, CodecError::InputExhausted { needed: 1, .. }));
        assert_eq!(reader.position(), BitPosition::new(0, 4));
    }

    #[test]
    fn oversized_read_is_rejected() {
        let mut reader = BitReader::new(&[0, 0, 0, 0, 0], BitOrder::LeastSignificantBitFirst);
        assert!(matches!(reader.read_bits(33), Err(CodecError::InvalidBitPattern { width: 33, .. })));
    }
}
