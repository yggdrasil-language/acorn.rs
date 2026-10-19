//! Canonical Huffman table construction and decoding.

use crate::{
    bit::{BitPosition, BitReader},
    error::CodecError,
};

const MAX_BITS: usize = 15;

/// Huffman decode table for deflate symbols.
///
/// Codes are stored in canonical order, symbols are grouped by code length.
/// Huffman codes are packed most-significant-bit first (RFC 1951 3.1.1), so
/// decoding accumulates stream bits into the high end of the working code.
pub struct HuffmanTable {
    counts: Vec<u16>,
    symbols: Vec<u16>,
    max_len: u8,
}

impl HuffmanTable {
    /// Builds an empty table that rejects every symbol.
    ///
    /// Deflate allows a dynamic block without any distance code as long as no
    /// length/distance pair is used, so this case must be representable.
    pub fn empty() -> Self {
        Self { counts: vec![0u16; MAX_BITS + 1], symbols: Vec::new(), max_len: 0 }
    }

    /// Builds a Huffman table from code lengths (0 = absent).
    ///
    /// The returned error carries no bit position because construction happens
    /// before any symbol is read. Callers map it onto the current read position.
    pub fn from_lengths(lengths: &[u8]) -> Result<Self, CodecError> {
        let mut counts = vec![0u16; MAX_BITS + 1];
        for &len in lengths {
            if len as usize > MAX_BITS {
                return Err(CodecError::InvalidHuffmanTree { position: BitPosition::new(0, 0) });
            }
            if len > 0 {
                counts[len as usize] += 1;
            }
        }

        let total = counts.iter().map(|&count| count as usize).sum::<usize>();
        if total == 0 {
            return Err(CodecError::InvalidHuffmanTree { position: BitPosition::new(0, 0) });
        }

        let mut left = 1i64;
        for bits in 1..=MAX_BITS {
            left = (left << 1) - counts[bits] as i64;
            if left < 0 {
                return Err(CodecError::InvalidHuffmanTree { position: BitPosition::new(0, 0) });
            }
        }

        let mut offsets = vec![0usize; MAX_BITS + 2];
        let mut offset = 0usize;
        for bits in 1..=MAX_BITS {
            offsets[bits] = offset;
            offset += counts[bits] as usize;
        }

        let mut symbols = vec![0u16; total];
        let mut max_len = 0u8;
        for (symbol, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            symbols[offsets[len as usize]] = symbol as u16;
            offsets[len as usize] += 1;
            max_len = max_len.max(len);
        }

        Ok(Self { counts, symbols, max_len })
    }

    /// Decodes one symbol from `reader`.
    pub fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, CodecError> {
        let mut code = 0u32;
        let mut first = 0u32;
        let mut index = 0usize;
        for len in 1..=self.max_len {
            code = (code << 1) | reader.read_bits(1)?;
            let count = self.counts[len as usize] as u32;
            if code >= first && code - first < count {
                return Ok(self.symbols[index + (code - first) as usize]);
            }
            index += count as usize;
            first = (first + count) << 1;
        }
        Err(CodecError::InvalidHuffmanTree { position: reader.position() })
    }
}

/// Fixed literal/length Huffman table (RFC 1951 section 3.2.6).
pub fn fixed_literal_table() -> HuffmanTable {
    let mut lengths = vec![0u8; 288];
    lengths[..=143].fill(8);
    lengths[144..=255].fill(9);
    lengths[256..=279].fill(7);
    lengths[280..=287].fill(8);
    HuffmanTable::from_lengths(&lengths).expect("fixed literal table")
}

/// Fixed distance Huffman table (RFC 1951 section 3.2.6).
pub fn fixed_distance_table() -> HuffmanTable {
    HuffmanTable::from_lengths(&[5u8; 32]).expect("fixed distance table")
}

/// Code length alphabet order for dynamic Huffman header.
pub const CODE_LENGTH_ORDER: [u8; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// Length lookup tables (RFC 1951 section 3.2.5).
pub const LENGTH_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];

pub const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

/// Distance lookup tables (RFC 1951 section 3.2.5).
pub const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193,
    12289, 16385, 24577,
];

pub const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

#[cfg(test)]
mod tests {
    use crate::{
        bit::{BitOrder, BitReader},
        compression::deflate::huffman::{HuffmanTable, fixed_distance_table, fixed_literal_table},
    };

    #[test]
    fn fixed_literal_table_builds() {
        let mut lengths = vec![0u8; 288];
        lengths[..=143].fill(8);
        lengths[144..=255].fill(9);
        lengths[256..=279].fill(7);
        lengths[280..=287].fill(8);
        HuffmanTable::from_lengths(&lengths).expect("full fixed literal table");
    }

    #[test]
    fn empty_table_rejects_every_symbol() {
        let table = HuffmanTable::empty();
        let mut reader = BitReader::new(&[0xFF, 0xFF], BitOrder::LeastSignificantBitFirst);
        assert!(table.decode(&mut reader).is_err());
    }

    #[test]
    fn oversubscribed_table_is_rejected() {
        assert!(HuffmanTable::from_lengths(&[1, 1, 1]).is_err());
    }

    #[test]
    fn fixed_tables_round_trip_known_symbols() {
        let literals = fixed_literal_table();
        let distances = fixed_distance_table();
        assert_eq!(literals.max_len, 9);
        assert_eq!(distances.max_len, 5);
        // `0x00` selects the fixed 7-bit code `0b0000000` (symbol 256, end of block).
        let mut reader = BitReader::new(&[0x00], BitOrder::LeastSignificantBitFirst);
        assert_eq!(literals.decode(&mut reader).expect("end of block"), 256);
    }
}
