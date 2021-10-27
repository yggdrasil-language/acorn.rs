//! Raw Deflate encoder (RFC 1951).

use std::{cmp::Reverse, collections::BinaryHeap};

use crate::bit::{BitOrder, BitWriter};

use super::huffman::{CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, LENGTH_BASE, LENGTH_EXTRA};

/// Deflate compression level.
///
/// The level only chooses block layout and match search effort. It never
/// changes the meaning of the output stream, so decoding is identical for every
/// level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeflateLevel {
    /// Emits stored blocks only: no compression, byte-aligned output.
    Store,
    /// Emits one fixed Huffman block with a short match search.
    Fast,
    /// Emits the smaller of a fixed and a dynamic Huffman block with a longer match search.
    #[default]
    Default,
}

/// Owns the encode side of the raw Deflate format.
///
/// `Default` falls back to a fixed Huffman block whenever a dynamic table would
/// need more than the RFC limits of 15 bits per symbol or 7 bits per code-length
/// symbol, so an emitted stream is always decodable.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeflateEncoder {
    level: DeflateLevel,
}

impl DeflateEncoder {
    /// Creates an encoder for `level`.
    #[inline]
    pub const fn new(level: DeflateLevel) -> Self {
        Self { level }
    }

    /// Returns the configured level.
    #[inline]
    pub const fn level(&self) -> DeflateLevel {
        self.level
    }

    /// Encodes `input` as a raw Deflate stream.
    pub fn encode(&self, input: &[u8]) -> Vec<u8> {
        match self.level {
            DeflateLevel::Store => encode_stored(input, true),
            DeflateLevel::Fast => encode(&tokens(input, FAST_SEARCH), BlockKind::Fixed, true).expect("fixed block"),
            DeflateLevel::Default => {
                let tokens = tokens(input, DEEP_SEARCH);
                let fixed = encode(&tokens, BlockKind::Fixed, true).expect("fixed block");
                match encode(&tokens, BlockKind::Dynamic, true) {
                    Some(dynamic) if dynamic.len() < fixed.len() => dynamic,
                    _ => fixed,
                }
            }
        }
    }
}

/// Largest number of bytes a single stored block may carry.
const MAX_STORED: usize = 65_535;

/// Sliding window size that a back-reference distance must fit into.
const WINDOW_SIZE: usize = 32 * 1024;

const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;

const HASH_BITS: usize = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;

/// Match search depth of the `Fast` level.
const FAST_SEARCH: usize = 32;
/// Match search depth of the `Default` level.
const DEEP_SEARCH: usize = 128;

/// Literal, end-of-block, and length symbols of the Deflate alphabet.
const LITERAL_SYMBOLS: usize = 286;
/// Distance symbols of the Deflate alphabet.
const DISTANCE_SYMBOLS: usize = 30;
/// Maximum code length of a literal or distance table (RFC 1951 section 3.2.7).
const MAX_CODE_BITS: u8 = 15;
/// Maximum code length of a code-length table (RFC 1951 section 3.2.7).
const MAX_CODE_LENGTH_BITS: u8 = 7;

/// Writes `input` as one or more stored blocks into `writer`.
fn write_stored(writer: &mut BitWriter, input: &[u8], final_block: bool) {
    let mut offset = 0usize;
    loop {
        let length = (input.len() - offset).min(MAX_STORED);
        let last = offset + length == input.len();

        let _ = writer.write_bits(u32::from(final_block && last), 1);
        let _ = writer.write_bits(0, 2);
        writer.align_to_byte();

        let len = length as u16;
        writer.write_byte(len as u8);
        writer.write_byte((len >> 8) as u8);
        writer.write_byte(!len as u8);
        writer.write_byte(((!len) >> 8) as u8);
        for &byte in &input[offset..offset + length] {
            writer.write_byte(byte);
        }

        offset += length;
        if last {
            return;
        }
    }
}

fn encode_stored(input: &[u8], final_block: bool) -> Vec<u8> {
    let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
    write_stored(&mut writer, input, final_block);
    writer.finish()
}

/// One LZ77 output token.
enum Token {
    /// A single literal byte.
    Literal(u8),
    /// A back-reference with `length` bytes at `distance` bytes back.
    Match { length: u32, distance: u32 },
}

/// Block layout of a candidate encoding.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    /// Fixed Huffman tables.
    Fixed,
    /// Tables derived from the token frequencies.
    Dynamic,
}

fn tokens(input: &[u8], chain_depth: usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut head = vec![usize::MAX; HASH_SIZE];
    let mut chain = vec![usize::MAX; input.len()];

    let mut pos = 0usize;
    while pos < input.len() {
        let (length, distance) = find_match(input, &head, &chain, pos, chain_depth);
        if length >= MIN_MATCH {
            tokens.push(Token::Match { length: length as u32, distance: distance as u32 });
            for index in pos..pos + length {
                insert_match(input, &mut head, &mut chain, index);
            }
            pos += length;
        }
        else {
            tokens.push(Token::Literal(input[pos]));
            insert_match(input, &mut head, &mut chain, pos);
            pos += 1;
        }
    }
    tokens
}

/// Writes `tokens` as one block into `writer`.
///
/// Returns `None` when the dynamic tables cannot be expressed inside the RFC
/// limits. Nothing is written in that case, so the caller can retry the same
/// token stream as a fixed block without corrupting the bit position.
///
/// `final_block` selects the `BFINAL` bit. Streaming encoders emit intermediate
/// blocks with `false` and terminate with `true`.
fn write_block(writer: &mut BitWriter, tokens: &[Token], kind: BlockKind, final_block: bool) -> Option<()> {
    let plan = match kind {
        BlockKind::Fixed => None,
        BlockKind::Dynamic => Some(DynamicPlan::build(tokens)?),
    };

    let _ = writer.write_bits(u32::from(final_block), 1);
    match &plan {
        Some(plan) => {
            let _ = writer.write_bits(2, 2);
            plan.write_header(writer);
            plan.write_tokens(writer, tokens);
            plan.write_literal_symbol(writer, 256);
        }
        None => {
            let _ = writer.write_bits(1, 2);
            for token in tokens {
                match token {
                    Token::Literal(byte) => write_symbol(writer, u16::from(*byte)),
                    Token::Match { length, distance } => {
                        let (index, extra, extra_bits) = length_index(*length);
                        write_symbol(writer, 257 + index as u16);
                        if extra_bits > 0 {
                            let _ = writer.write_bits(extra, extra_bits);
                        }
                        let (distance_index, extra, extra_bits) = distance_index(*distance);
                        write_code(writer, distance_index as u32, 5);
                        if extra_bits > 0 {
                            let _ = writer.write_bits(extra, extra_bits);
                        }
                    }
                }
            }
            write_symbol(writer, 256);
        }
    }

    Some(())
}

fn encode(tokens: &[Token], kind: BlockKind, final_block: bool) -> Option<Vec<u8>> {
    let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
    write_block(&mut writer, tokens, kind, final_block)?;
    Some(writer.finish())
}

fn hash_bytes(input: &[u8], pos: usize) -> usize {
    let first = input[pos] as usize;
    let second = input[pos + 1] as usize;
    let third = input[pos + 2] as usize;
    ((first << 10) ^ (second << 5) ^ third) & (HASH_SIZE - 1)
}

fn insert_match(input: &[u8], head: &mut [usize], chain: &mut [usize], pos: usize) {
    if pos + MIN_MATCH > input.len() {
        return;
    }
    let hash = hash_bytes(input, pos);
    chain[pos] = head[hash];
    head[hash] = pos;
}

fn find_match(input: &[u8], head: &[usize], chain: &[usize], pos: usize, depth: usize) -> (usize, usize) {
    if pos + MIN_MATCH > input.len() {
        return (0, 0);
    }

    let limit = pos.saturating_sub(WINDOW_SIZE);
    let max_length = (input.len() - pos).min(MAX_MATCH);
    let mut best_length = 0usize;
    let mut best_distance = 0usize;
    let mut candidate = head[hash_bytes(input, pos)];
    let mut remaining = depth;

    while candidate != usize::MAX && candidate >= limit && remaining > 0 {
        // The quick check rejects candidates that cannot beat the current best.
        if input[candidate + best_length] == input[pos + best_length] {
            let mut length = 0usize;
            while length < max_length && input[candidate + length] == input[pos + length] {
                length += 1;
            }
            if length > best_length {
                best_length = length;
                best_distance = pos - candidate;
                if length == max_length {
                    break;
                }
            }
        }
        candidate = chain[candidate];
        remaining -= 1;
    }

    if best_length >= MIN_MATCH { (best_length, best_distance) } else { (0, 0) }
}

/// Fixed literal/length code and width for `symbol` (RFC 1951 section 3.2.6).
pub(crate) fn fixed_symbol_code(symbol: u16) -> (u32, u8) {
    match symbol {
        0..=143 => (0x30 + u32::from(symbol), 8),
        144..=255 => (0x190 + (u32::from(symbol) - 144), 9),
        256..=279 => (u32::from(symbol) - 256, 7),
        _ => (0xC0 + (u32::from(symbol) - 280), 8),
    }
}

pub(crate) fn write_symbol(writer: &mut BitWriter, symbol: u16) {
    let (code, length) = fixed_symbol_code(symbol);
    write_code(writer, code, length);
}

/// Length symbol index, extra value, and extra width for a match length.
pub(crate) fn length_index(length: u32) -> (usize, u32, u8) {
    let mut index = LENGTH_BASE.len() - 1;
    while index > 0 && u32::from(LENGTH_BASE[index]) > length {
        index -= 1;
    }
    (index, length - u32::from(LENGTH_BASE[index]), LENGTH_EXTRA[index])
}

/// Distance symbol index, extra value, and extra width for a match distance.
pub(crate) fn distance_index(distance: u32) -> (usize, u32, u8) {
    let mut index = DIST_BASE.len() - 1;
    while index > 0 && u32::from(DIST_BASE[index]) > distance {
        index -= 1;
    }
    (index, distance - u32::from(DIST_BASE[index]), DIST_EXTRA[index])
}

/// Writes a Huffman code, which travels most-significant bit first.
pub(crate) fn write_code(writer: &mut BitWriter, code: u32, length: u8) {
    let mut reversed = 0u32;
    for index in 0..length {
        reversed = (reversed << 1) | ((code >> index) & 1);
    }
    let _ = writer.write_bits(reversed, length);
}

/// Canonical code and width per symbol for a code-length vector.
fn canonical_codes(lengths: &[u8]) -> Vec<(u32, u8)> {
    let mut counts = [0u32; (MAX_CODE_BITS + 1) as usize];
    for &length in lengths {
        if length > 0 {
            counts[length as usize] += 1;
        }
    }
    let mut next = [0u32; (MAX_CODE_BITS + 1) as usize];
    let mut code = 0u32;
    for bits in 1..=MAX_CODE_BITS as usize {
        code = (code + counts[bits - 1]) << 1;
        next[bits] = code;
    }
    lengths
        .iter()
        .map(|&length| {
            if length == 0 {
                (0, 0)
            }
            else {
                let assigned = next[length as usize];
                next[length as usize] += 1;
                (assigned, length)
            }
        })
        .collect()
}

/// Huffman code lengths for `frequencies`, or `None` when they exceed `max_bits`.
///
/// A single used symbol gets a one-bit code, which Deflate accepts as an
/// incomplete tree.
fn huffman_lengths(frequencies: &[u32], max_bits: u8) -> Option<Vec<u8>> {
    let mut lengths = vec![0u8; frequencies.len()];
    let leaves: Vec<usize> = (0..frequencies.len()).filter(|&index| frequencies[index] > 0).collect();
    match leaves.len() {
        0 => return Some(lengths),
        1 => {
            lengths[leaves[0]] = 1;
            return Some(lengths);
        }
        _ => {}
    }

    let mut heap: BinaryHeap<(Reverse<u32>, Reverse<usize>, usize)> = BinaryHeap::new();
    for (order, &symbol) in leaves.iter().enumerate() {
        heap.push((Reverse(frequencies[symbol]), Reverse(order), symbol));
    }

    let mut parents = vec![usize::MAX; frequencies.len() + leaves.len()];
    let mut next_order = leaves.len();
    while heap.len() > 1 {
        let (Reverse(first), Reverse(_), left) = heap.pop().expect("two nodes");
        let (Reverse(second), Reverse(_), right) = heap.pop().expect("two nodes");
        let parent = frequencies.len() + (next_order - leaves.len());
        parents[left] = parent;
        parents[right] = parent;
        heap.push((Reverse(first + second), Reverse(next_order), parent));
        next_order += 1;
    }

    for &leaf in &leaves {
        let mut depth = 0u8;
        let mut node = leaf;
        while parents[node] != usize::MAX {
            node = parents[node];
            depth += 1;
        }
        if depth > max_bits {
            return None;
        }
        lengths[leaf] = depth;
    }
    Some(lengths)
}

/// Dynamic Huffman tables derived from the token stream.
struct DynamicPlan {
    literal_lengths: Vec<u8>,
    distance_lengths: Vec<u8>,
    literal_codes: Vec<(u32, u8)>,
    distance_codes: Vec<(u32, u8)>,
    code_length_lengths: Vec<u8>,
    code_length_codes: Vec<(u32, u8)>,
    code_length_sequence: Vec<(u8, u32, u8)>,
    code_length_count: usize,
}

impl DynamicPlan {
    fn build(tokens: &[Token]) -> Option<Self> {
        let mut literal_frequencies = vec![0u32; LITERAL_SYMBOLS];
        let mut distance_frequencies = vec![0u32; DISTANCE_SYMBOLS];
        literal_frequencies[256] = 1;
        for token in tokens {
            match token {
                Token::Literal(byte) => literal_frequencies[usize::from(*byte)] += 1,
                Token::Match { length, distance } => {
                    let (index, _, _) = length_index(*length);
                    literal_frequencies[257 + index] += 1;
                    let (index, _, _) = distance_index(*distance);
                    distance_frequencies[index] += 1;
                }
            }
        }

        let literal_lengths = huffman_lengths(&literal_frequencies, MAX_CODE_BITS)?;
        let distance_lengths = if distance_frequencies.iter().all(|&frequency| frequency == 0) {
            vec![0u8; 1]
        }
        else {
            huffman_lengths(&distance_frequencies, MAX_CODE_BITS)?
        };

        let literal_count = last_nonzero(&literal_lengths).map_or(257, |index| (index + 1).max(257));
        let distance_count = last_nonzero(&distance_lengths).map_or(1, |index| index + 1);
        let literal_lengths = literal_lengths[..literal_count].to_vec();
        let distance_lengths = distance_lengths[..distance_count].to_vec();

        let mut combined = literal_lengths.clone();
        combined.extend_from_slice(&distance_lengths);
        let (code_length_sequence, code_length_frequencies) = run_length_encode(&combined);

        let code_length_lengths = huffman_lengths(&code_length_frequencies, MAX_CODE_LENGTH_BITS)?;
        let code_length_codes = canonical_codes(&code_length_lengths);
        let code_length_count = CODE_LENGTH_ORDER
            .iter()
            .rposition(|&symbol| code_length_lengths[symbol as usize] > 0)
            .map_or(4, |index| (index + 1).max(4));

        Some(Self {
            literal_codes: canonical_codes(&literal_lengths),
            distance_codes: canonical_codes(&distance_lengths),
            literal_lengths,
            distance_lengths,
            code_length_lengths,
            code_length_codes,
            code_length_sequence,
            code_length_count,
        })
    }

    fn write_header(&self, writer: &mut BitWriter) {
        let _ = writer.write_bits((self.literal_lengths.len() - 257) as u32, 5);
        let _ = writer.write_bits((self.distance_lengths.len() - 1) as u32, 5);
        let _ = writer.write_bits((self.code_length_count - 4) as u32, 4);
        for index in 0..self.code_length_count {
            let symbol = CODE_LENGTH_ORDER[index] as usize;
            let _ = writer.write_bits(u32::from(self.code_length_lengths[symbol]), 3);
        }
        for &(symbol, extra, extra_bits) in &self.code_length_sequence {
            let (code, length) = self.code_length_codes[symbol as usize];
            write_code(writer, code, length);
            if extra_bits > 0 {
                let _ = writer.write_bits(extra, extra_bits);
            }
        }
    }

    fn write_tokens(&self, writer: &mut BitWriter, tokens: &[Token]) {
        for token in tokens {
            match token {
                Token::Literal(byte) => self.write_literal_symbol(writer, u16::from(*byte)),
                Token::Match { length, distance } => {
                    let (index, extra, extra_bits) = length_index(*length);
                    self.write_literal_symbol(writer, 257 + index as u16);
                    if extra_bits > 0 {
                        let _ = writer.write_bits(extra, extra_bits);
                    }
                    let (index, extra, extra_bits) = distance_index(*distance);
                    let (code, length) = self.distance_codes[index];
                    write_code(writer, code, length);
                    if extra_bits > 0 {
                        let _ = writer.write_bits(extra, extra_bits);
                    }
                }
            }
        }
    }

    fn write_literal_symbol(&self, writer: &mut BitWriter, symbol: u16) {
        let (code, length) = self.literal_codes[usize::from(symbol)];
        write_code(writer, code, length);
    }
}

fn last_nonzero(lengths: &[u8]) -> Option<usize> {
    lengths.iter().rposition(|&length| length > 0)
}

/// Run-length encodes code lengths with the repeat symbols `16`, `17`, and `18`.
fn run_length_encode(lengths: &[u8]) -> (Vec<(u8, u32, u8)>, [u32; 19]) {
    let mut sequence = Vec::new();
    let mut frequencies = [0u32; 19];

    let mut index = 0usize;
    while index < lengths.len() {
        let length = lengths[index];
        let mut run = 1usize;
        while index + run < lengths.len() && lengths[index + run] == length {
            run += 1;
        }

        if length == 0 {
            let mut remaining = run;
            while remaining >= 11 {
                let count = remaining.min(138);
                sequence.push((18, (count - 11) as u32, 7));
                frequencies[18] += 1;
                remaining -= count;
            }
            while remaining >= 3 {
                let count = remaining.min(10);
                sequence.push((17, (count - 3) as u32, 3));
                frequencies[17] += 1;
                remaining -= count;
            }
            for _ in 0..remaining {
                sequence.push((0, 0, 0));
                frequencies[0] += 1;
            }
        }
        else {
            sequence.push((length, 0, 0));
            frequencies[length as usize] += 1;

            let mut remaining = run - 1;
            while remaining >= 3 {
                let count = remaining.min(6);
                sequence.push((16, (count - 3) as u32, 2));
                frequencies[16] += 1;
                remaining -= count;
            }
            for _ in 0..remaining {
                sequence.push((length, 0, 0));
                frequencies[length as usize] += 1;
            }
        }

        index += run;
    }

    (sequence, frequencies)
}

/// Incremental raw Deflate encoder.
///
/// Every [`DeflateStreamEncoder::feed`] call is emitted as one non-final block,
/// and [`DeflateStreamEncoder::finish`] writes an empty final block that closes
/// the stream. Blocks share one bit cursor, so no padding bits separate them.
///
/// Matches never cross a feed boundary and no window state is retained between
/// calls, so the caller chooses the trade between chunk size and compression
/// ratio. `Default` uses a dynamic table whenever the RFC limits allow one and
/// falls back to the fixed table otherwise, without comparing the two encodings
/// byte for byte. Peak memory is bounded by the largest chunk passed to `feed`.
#[derive(Debug, Clone)]
pub struct DeflateStreamEncoder {
    level: DeflateLevel,
    writer: BitWriter,
    finished: bool,
}

impl DeflateStreamEncoder {
    /// Creates a streaming encoder for `level`.
    #[inline]
    pub fn new(level: DeflateLevel) -> Self {
        Self { level, writer: BitWriter::new(BitOrder::LeastSignificantBitFirst), finished: false }
    }

    /// Returns the configured level.
    #[inline]
    pub const fn level(&self) -> DeflateLevel {
        self.level
    }

    /// Returns whether the terminal block has been emitted.
    #[inline]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// Compresses one bounded chunk into newly emitted whole bytes.
    ///
    /// Nothing is emitted for an empty chunk or after `finish`. A partially
    /// filled byte stays inside the encoder so the next block continues at the
    /// same bit position.
    pub fn feed(&mut self, input: &[u8]) -> Vec<u8> {
        if self.finished || input.is_empty() {
            return Vec::new();
        }
        match self.level {
            DeflateLevel::Store => write_stored(&mut self.writer, input, false),
            DeflateLevel::Fast => {
                let tokens = tokens(input, FAST_SEARCH);
                write_block(&mut self.writer, &tokens, BlockKind::Fixed, false).expect("fixed block");
            }
            DeflateLevel::Default => {
                let tokens = tokens(input, DEEP_SEARCH);
                if write_block(&mut self.writer, &tokens, BlockKind::Dynamic, false).is_none() {
                    write_block(&mut self.writer, &tokens, BlockKind::Fixed, false).expect("fixed block");
                }
            }
        }
        self.writer.take_bytes()
    }

    /// Emits the terminal block exactly once and releases the bit cursor.
    pub fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut writer = std::mem::replace(&mut self.writer, BitWriter::new(BitOrder::LeastSignificantBitFirst));
        match self.level {
            DeflateLevel::Store => write_stored(&mut writer, &[], true),
            DeflateLevel::Fast | DeflateLevel::Default => {
                write_block(&mut writer, &[], BlockKind::Fixed, true).expect("fixed block");
            }
        }
        writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockKind, DEEP_SEARCH, DeflateEncoder, DeflateLevel, DeflateStreamEncoder, encode, tokens};

    #[test]
    fn stored_blocks_split_at_the_length_limit() {
        let payload = vec![b'a'; 70_000];
        let encoded = DeflateEncoder::new(DeflateLevel::Store).encode(&payload);
        assert!(encoded.len() > payload.len());
    }

    #[test]
    fn fixed_encoding_finds_repetition() {
        let payload = b"abcdefgh".repeat(64);
        let encoded = DeflateEncoder::new(DeflateLevel::Fast).encode(&payload);
        assert!(encoded.len() < payload.len() / 2);
    }

    #[test]
    fn dynamic_encoding_beats_fixed_on_skewed_input() {
        let payload = b"the quick brown fox jumps over the lazy dog. ".repeat(60);
        let tokens = tokens(&payload, DEEP_SEARCH);
        let dynamic = encode(&tokens, BlockKind::Dynamic, true).expect("dynamic tables fit");
        let fixed = encode(&tokens, BlockKind::Fixed, true).expect("fixed block");
        assert_eq!((dynamic[0] >> 1) & 3, 2, "dynamic block type");
        assert!(dynamic.len() < fixed.len());
    }

    #[test]
    fn streaming_encoder_produces_a_decodable_stream() {
        let payload = b"streamed blocks keep memory flat while matching. ".repeat(500);
        for level in [DeflateLevel::Store, DeflateLevel::Fast, DeflateLevel::Default] {
            let mut encoder = DeflateStreamEncoder::new(level);
            let mut encoded = Vec::new();
            for chunk in payload.chunks(257) {
                encoded.extend_from_slice(&encoder.feed(chunk));
            }
            encoded.extend_from_slice(&encoder.finish());
            let decoded = crate::compression::Deflate::decode(&encoded, crate::limits::CodecLimits::new())
                .unwrap_or_else(|error| panic!("level {level:?} stream is invalid: {error}"));
            assert_eq!(decoded.value, payload, "level {level:?}");
        }
    }
}
