//! Explicit Deflate block emission for fixed-point orchestration.
//!
//! [`Deflate`](super::Deflate) compresses a whole input, so it owns the block
//! layout of the stream. Callers that build a stream block by block — recursive fixed-point templates, for
//! example — need to choose the block sequence, the `BFINAL` position, and the
//! byte alignment themselves.
//!
//! This module owns the RFC 1951 write primitives for that second kind of
//! caller. It never chooses a layout: [`write_block`] writes exactly one block
//! body at the writer's current bit position and leaves alignment to the
//! caller.

use crate::{
    bit::{BitPosition, BitWriter},
    error::CodecError,
};

use super::encoder::{distance_index, length_index, write_code, write_symbol};

/// Largest payload a single stored block can carry (RFC 1951 section 3.2.4).
const MAX_STORED: usize = 65_535;

/// Smallest match length a Deflate length symbol can express.
const MIN_MATCH: u16 = 3;
/// Largest match length a Deflate length symbol can express.
const MAX_MATCH: u16 = 258;
/// Largest back-reference distance a Deflate distance symbol can express.
const MAX_DISTANCE: u16 = 32_768;

/// One symbol inside a fixed Huffman block.
///
/// Codes are the RFC 1951 section 3.2.6 fixed codes. The block terminator is
/// appended by [`write_fixed_block`], so callers never emit symbol `256`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    /// Literal byte, `0..=255`.
    Literal(u8),
    /// Back-reference with `length` in `3..=258` and `distance` in `1..=32768`.
    Match {
        /// Match length in bytes.
        length: u16,
        /// Back-reference distance in bytes.
        distance: u16,
    },
}

/// Stored (uncompressed, `BTYPE=00`) block payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoredBlock {
    data: Vec<u8>,
}

impl StoredBlock {
    /// Creates a stored block from its payload.
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self { data: data.into() }
    }

    /// Returns the payload bytes.
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the payload length.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the payload is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Writes this block at `writer`'s current bit position.
    ///
    /// Alignment is the caller's decision.
    pub fn write_into(&self, writer: &mut BitWriter, final_block: bool) -> Result<(), CodecError> {
        write_stored_block(writer, &self.data, final_block)
    }
}

/// Fixed Huffman (`BTYPE=01`) block symbol sequence.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FixedBlock {
    symbols: Vec<Symbol>,
}

impl FixedBlock {
    /// Creates a fixed block from an explicit symbol sequence.
    pub fn new(symbols: Vec<Symbol>) -> Self {
        Self { symbols }
    }

    /// Creates a fixed block whose symbols are all literals.
    pub fn from_literals(bytes: &[u8]) -> Self {
        Self { symbols: bytes.iter().map(|&byte| Symbol::Literal(byte)).collect() }
    }

    /// Returns the symbol sequence, without the terminator.
    #[inline]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Returns the symbol count, without the terminator.
    #[inline]
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Whether the block carries no symbols.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Writes this block at `writer`'s current bit position.
    ///
    /// Alignment is the caller's decision.
    pub fn write_into(&self, writer: &mut BitWriter, final_block: bool) -> Result<(), CodecError> {
        write_fixed_block(writer, &self.symbols, final_block)
    }
}

/// One Deflate block body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeflateBlock {
    /// Stored / uncompressed block (`BTYPE=00`).
    Stored(StoredBlock),
    /// Fixed Huffman block (`BTYPE=01`).
    Fixed(FixedBlock),
}

impl DeflateBlock {
    /// Writes this block at `writer`'s current bit position.
    pub fn write_into(&self, writer: &mut BitWriter, final_block: bool) -> Result<(), CodecError> {
        write_block(writer, self, final_block)
    }
}

/// Ordered block sequence with an explicit `BFINAL` flag per block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeflateBlockPlan {
    blocks: Vec<(DeflateBlock, bool)>,
}

impl DeflateBlockPlan {
    /// Creates an empty plan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a block with its finality flag.
    pub fn push(&mut self, block: DeflateBlock, final_block: bool) {
        self.blocks.push((block, final_block));
    }

    /// Returns the planned `(block, final_block)` pairs in stream order.
    #[inline]
    pub fn blocks(&self) -> &[(DeflateBlock, bool)] {
        &self.blocks
    }

    /// Whether the plan holds no blocks.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Encodes the plan as a raw Deflate stream.
    ///
    /// The blocks are written back to back at the writer's bit position. No
    /// padding is inserted between them, because a reader looks for the next
    /// block header immediately after the previous block's last bit. Only a
    /// stored block aligns, and it aligns *inside* itself.
    ///
    /// The plan is encoded exactly as planned: this entry point never inserts,
    /// drops, or reorders a block, and it does not require the final flag to sit
    /// on the last entry.
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        if self.blocks.is_empty() {
            return Err(CodecError::InvalidBlockHeader { position: BitPosition::new(0, 0) });
        }
        let mut writer = BitWriter::new(crate::bit::BitOrder::LeastSignificantBitFirst);
        for (block, final_block) in &self.blocks {
            write_block(&mut writer, block, *final_block)?;
        }
        Ok(writer.finish())
    }
}

/// Writes one Deflate block at `writer`'s current bit position.
///
/// The primitive writes the block header, the block body, and (for a fixed
/// block) the end-of-block symbol. It performs no byte alignment and appends no
/// terminator of its own, so a caller can compose several blocks inside one
/// bit-continuous stream and read the resulting position from
/// [`BitWriter::position`].
pub fn write_block(writer: &mut BitWriter, block: &DeflateBlock, final_block: bool) -> Result<(), CodecError> {
    match block {
        DeflateBlock::Stored(stored) => stored.write_into(writer, final_block),
        DeflateBlock::Fixed(fixed) => fixed.write_into(writer, final_block),
    }
}

/// Writes a stored (`BTYPE=00`) block at `writer`'s current bit position.
///
/// The payload must fit the 16-bit `LEN` field. Longer payloads are rejected
/// instead of being split, because splitting is a layout decision that belongs
/// to the caller.
pub fn write_stored_block(writer: &mut BitWriter, data: &[u8], final_block: bool) -> Result<(), CodecError> {
    if data.len() > MAX_STORED {
        return Err(CodecError::OutputLimitExceeded { produced: data.len() as u64, limit: MAX_STORED as u64 });
    }
    let _ = writer.write_bits(u32::from(final_block), 1);
    let _ = writer.write_bits(0b00, 2);
    writer.align_to_byte();
    let length = data.len() as u16;
    writer.write_byte(length as u8);
    writer.write_byte((length >> 8) as u8);
    let complement = !length;
    writer.write_byte(complement as u8);
    writer.write_byte((complement >> 8) as u8);
    for &byte in data {
        writer.write_byte(byte);
    }
    Ok(())
}

/// Writes a fixed Huffman (`BTYPE=01`) block at `writer`'s current bit position.
///
/// The end-of-block symbol is appended automatically. Match symbols are checked
/// for length and distance range only: whether a distance can reach the bytes
/// already produced is knowledge the caller has and this primitive does not.
pub fn write_fixed_block(writer: &mut BitWriter, symbols: &[Symbol], final_block: bool) -> Result<(), CodecError> {
    let _ = writer.write_bits(u32::from(final_block), 1);
    let _ = writer.write_bits(0b01, 2);
    for symbol in symbols {
        match *symbol {
            Symbol::Literal(byte) => write_symbol(writer, u16::from(byte)),
            Symbol::Match { length, distance } => {
                if !(MIN_MATCH..=MAX_MATCH).contains(&length) {
                    return Err(CodecError::InvalidMatchLength { position: writer.position(), length });
                }
                if distance == 0 {
                    return Err(CodecError::InvalidBackReference { position: writer.position(), distance: 0, available: 0 });
                }
                if distance > MAX_DISTANCE {
                    return Err(CodecError::DistanceOutOfWindow {
                        position: writer.position(),
                        distance: u32::from(distance),
                        window: u32::from(MAX_DISTANCE),
                    });
                }
                let (index, extra, extra_bits) = length_index(u32::from(length));
                write_symbol(writer, 257 + index as u16);
                if extra_bits > 0 {
                    let _ = writer.write_bits(extra, extra_bits);
                }
                let (index, extra, extra_bits) = distance_index(u32::from(distance));
                write_code(writer, index as u32, 5);
                if extra_bits > 0 {
                    let _ = writer.write_bits(extra, extra_bits);
                }
            }
        }
    }
    write_symbol(writer, 256);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        bit::{BitOrder, BitReader, BitWriter},
        compression::Deflate,
        error::CodecError,
        limits::CodecLimits,
    };

    use super::{DeflateBlock, DeflateBlockPlan, FixedBlock, StoredBlock, Symbol, write_block};

    fn decode(raw: &[u8], limit: u64) -> Vec<u8> {
        let limits = CodecLimits { max_output_bytes: Some(limit), ..CodecLimits::default() };
        Deflate::decode(raw, limits).expect("decode").value
    }

    #[test]
    fn plan_with_a_single_final_stored_block_round_trips() {
        let mut plan = DeflateBlockPlan::new();
        plan.push(DeflateBlock::Stored(StoredBlock::new(b"hello archive".to_vec())), true);
        let raw = plan.encode().expect("encode");
        assert_eq!(decode(&raw, 1024), b"hello archive");
    }

    #[test]
    fn literal_and_match_symbols_reproduce_their_input() {
        let payload = b"abcabcabc";
        let mut plan = DeflateBlockPlan::new();
        plan.push(
            DeflateBlock::Fixed(FixedBlock::new(vec![
                Symbol::Literal(b'a'),
                Symbol::Literal(b'b'),
                Symbol::Literal(b'c'),
                Symbol::Match { length: 6, distance: 3 },
            ])),
            true,
        );
        let raw = plan.encode().expect("encode");
        assert_eq!(decode(&raw, 64), payload);
    }

    #[test]
    fn write_block_leaves_alignment_and_finality_to_the_caller() {
        let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
        write_block(&mut writer, &DeflateBlock::Fixed(FixedBlock::from_literals(b"x")), false).expect("first block");
        assert!(!writer.is_aligned(), "a fixed block does not align on its own");
        let after_first = writer.position();
        write_block(&mut writer, &DeflateBlock::Stored(StoredBlock::new(Vec::new())), true).expect("second block");
        assert!(after_second_bigger(&writer, &after_first));
        assert_eq!(decode(&writer.finish(), 64), b"x");
    }

    fn after_second_bigger(writer: &BitWriter, before: &crate::bit::BitPosition) -> bool {
        writer.position().total_bits() > before.total_bits()
    }

    #[test]
    fn match_symbols_outside_the_rfc_range_are_rejected() {
        for symbol in [
            Symbol::Match { length: 2, distance: 1 },
            Symbol::Match { length: 259, distance: 1 },
            Symbol::Match { length: 3, distance: 0 },
            Symbol::Match { length: 3, distance: 32_769 },
        ] {
            let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
            let error = write_block(&mut writer, &DeflateBlock::Fixed(FixedBlock::new(vec![symbol])), false)
                .expect_err("out-of-range match");
            assert!(
                matches!(
                    error,
                    CodecError::InvalidMatchLength { .. }
                        | CodecError::InvalidBackReference { .. }
                        | CodecError::DistanceOutOfWindow { .. }
                ),
                "unexpected error {error:?}"
            );
        }
    }

    #[test]
    fn oversized_stored_payload_is_rejected_instead_of_split() {
        let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
        let data = vec![0u8; 65_536];
        let error = write_block(&mut writer, &DeflateBlock::Stored(StoredBlock::new(data)), true)
            .expect_err("oversized stored payload");
        assert!(matches!(error, CodecError::OutputLimitExceeded { limit: 65_535, .. }));
    }

    #[test]
    fn empty_plan_is_rejected() {
        assert!(DeflateBlockPlan::new().encode().is_err());
    }

    #[test]
    fn block_writes_round_trip_through_the_reader() {
        let mut plan = DeflateBlockPlan::new();
        plan.push(DeflateBlock::Stored(StoredBlock::new(b"stored".to_vec())), false);
        plan.push(DeflateBlock::Fixed(FixedBlock::from_literals(b"fixed")), false);
        plan.push(DeflateBlock::Stored(StoredBlock::new(vec![b'z'; 40])), true);
        let raw = plan.encode().expect("encode");
        assert_eq!(decode(&raw, 1024), b"storedfixedzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
    }

    #[test]
    fn whole_input_encoder_and_block_plan_agree_on_a_stored_stream() {
        let payload = b"the quick brown fox".to_vec();
        let encoded = Deflate::encode(&payload, super::super::DeflateLevel::Store);
        let mut plan = DeflateBlockPlan::new();
        plan.push(DeflateBlock::Stored(StoredBlock::new(payload.clone())), true);
        assert_eq!(plan.encode().expect("encode"), encoded);
    }

    #[test]
    fn writer_position_tracks_the_block_bit_length() {
        let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
        let before = writer.position();
        write_block(&mut writer, &DeflateBlock::Fixed(FixedBlock::from_literals(b"abc")), false).expect("block");
        // Three 8-bit literals plus a 7-bit end-of-block symbol inside a 3-bit header.
        assert_eq!(writer.position().total_bits() - before.total_bits(), 3 + 24 + 7);
        assert_eq!(BitReader::new(&writer.finish(), BitOrder::LeastSignificantBitFirst).read_bits(1).expect("bfinal"), 0);
    }
}
