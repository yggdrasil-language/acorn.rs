//! Bit-level output into a byte buffer.

use crate::{
    bit::{BitOrder, BitPosition},
    error::CodecError,
};

/// Maximum number of bits a single write may request.
pub const MAX_WRITE_BITS: u8 = 32;

/// Bit writer that packs fields into whole bytes.
///
/// The writer keeps one partially filled byte. [`BitWriter::align_to_byte`] and
/// [`BitWriter::finish`] pad the remaining bits with zeroes.
#[derive(Debug, Clone)]
pub struct BitWriter {
    order: BitOrder,
    bytes: Vec<u8>,
    current: u8,
    bit_count: u8,
}

impl BitWriter {
    /// Creates an empty writer using `order`.
    pub const fn new(order: BitOrder) -> Self {
        Self { order, bytes: Vec::new(), current: 0, bit_count: 0 }
    }

    /// Returns the configured bit order.
    #[inline]
    pub const fn order(&self) -> BitOrder {
        self.order
    }

    /// Returns the position of the next bit to write.
    #[inline]
    pub fn position(&self) -> BitPosition {
        BitPosition::new(self.bytes.len() as u64, self.bit_count)
    }

    /// Whether the writer sits on a byte boundary.
    #[inline]
    pub fn is_aligned(&self) -> bool {
        self.bit_count == 0
    }

    /// Writes one bit.
    #[inline]
    pub fn write_bit(&mut self, bit: u32) {
        self.push_bit(bit);
    }

    /// Writes the low `count` bits of `value` in the configured order.
    pub fn write_bits(&mut self, value: u32, count: u8) -> Result<(), CodecError> {
        if count > MAX_WRITE_BITS {
            return Err(CodecError::InvalidBitPattern { offset: self.position().byte_offset as usize, width: count });
        }
        for index in 0..count {
            let bit = match self.order {
                BitOrder::LeastSignificantBitFirst => value >> index,
                BitOrder::MostSignificantBitFirst => value >> (count - 1 - index),
            };
            self.push_bit(bit);
        }
        Ok(())
    }

    /// Writes a whole byte, padding the current byte first.
    pub fn write_byte(&mut self, byte: u8) {
        self.align_to_byte();
        self.bytes.push(byte);
    }

    /// Pads the current byte with zeroes so the next write starts on a boundary.
    pub fn align_to_byte(&mut self) {
        if self.bit_count > 0 {
            self.bytes.push(self.current);
            self.current = 0;
            self.bit_count = 0;
        }
    }

    /// Pads the final byte and returns the packed bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.align_to_byte();
        self.bytes
    }

    fn push_bit(&mut self, bit: u32) {
        let shift = match self.order {
            BitOrder::LeastSignificantBitFirst => self.bit_count,
            BitOrder::MostSignificantBitFirst => 7 - self.bit_count,
        };
        self.current |= ((bit & 1) as u8) << shift;
        self.bit_count += 1;
        if self.bit_count == 8 {
            self.bytes.push(self.current);
            self.current = 0;
            self.bit_count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bit::{BitOrder, BitPosition, BitReader, BitWriter},
        error::CodecError,
    };

    #[test]
    fn least_significant_bit_first_writes_byte_boundaries() {
        let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
        writer.write_bits(0b110, 3).expect("low bits");
        writer.write_bits(0b110_110, 6).expect("spanning bits");
        assert_eq!(writer.position(), BitPosition::new(1, 1));
        assert_eq!(writer.finish(), vec![0b1011_0110, 0b0000_0001]);
    }

    #[test]
    fn most_significant_bit_first_writes_field_values() {
        let mut writer = BitWriter::new(BitOrder::MostSignificantBitFirst);
        writer.write_bits(0b1011, 4).expect("high nibble");
        writer.write_bits(0b0110, 4).expect("low nibble");
        assert_eq!(writer.finish(), vec![0b1011_0110]);
    }

    #[test]
    fn writer_output_round_trips_through_reader() {
        for order in [BitOrder::LeastSignificantBitFirst, BitOrder::MostSignificantBitFirst] {
            let mut writer = BitWriter::new(order);
            writer.write_bits(0b101, 3).expect("first");
            writer.write_bits(0x3FFF, 14).expect("second");
            writer.write_bit(1);
            let bytes = writer.finish();
            let mut reader = BitReader::new(&bytes, order);
            assert_eq!(reader.read_bits(3).expect("first"), 0b101);
            assert_eq!(reader.read_bits(14).expect("second"), 0x3FFF);
            assert_eq!(reader.read_bit().expect("last"), 1);
        }
    }

    #[test]
    fn whole_byte_writes_stay_byte_aligned() {
        let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
        writer.write_bits(0b1, 1).expect("bit");
        writer.write_byte(0xAB);
        assert_eq!(writer.finish(), vec![0b0000_0001, 0xAB]);
    }

    #[test]
    fn oversized_write_is_rejected() {
        let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
        assert!(matches!(writer.write_bits(0, 33), Err(CodecError::InvalidBitPattern { width: 33, .. })));
    }
}
