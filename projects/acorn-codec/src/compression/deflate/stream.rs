//! Resumable raw Deflate decoding.
//!
//! The one-shot decoder in [`super::decoder`] owns the complete input slice.
//! This module keeps the same block and Huffman rules but stores the bit
//! cursor, sliding window, block state and pending match in a struct, so a
//! caller can feed bounded chunks and pull bounded output without ever holding
//! the stream in memory.
//!
//! An attempt that runs out of input is rolled back to the last safe point.
//! Bytes are only committed when a whole header or token has been decoded, so
//! a later push resumes exactly where the previous read stopped.

use crate::{
    bit::{BitPosition, BitSource, MAX_READ_BITS},
    compression::{
        input::{InputQueue, exhausted},
        stream::StreamChunk,
    },
    error::CodecError,
    facts::StreamState,
    limits::CodecLimits,
};

use super::{
    huffman::{
        CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, HuffmanTable, LENGTH_BASE, LENGTH_EXTRA, fixed_distance_table,
        fixed_literal_table,
    },
    window::SlidingWindow,
};

/// Bytes read from the queue per bit-cursor cache refill.
const BIT_CACHE_BYTES: u64 = 8;

/// Outcome of one whole Deflate token.
#[derive(Debug, Clone, Copy)]
enum Token {
    /// One literal byte.
    Literal(u8),
    /// A back-reference to bytes already produced.
    Match { length: u32, distance: u32 },
    /// End of the current block.
    EndOfBlock,
}

/// A back-reference that could not be written into the caller buffer.
#[derive(Debug, Clone, Copy)]
struct PendingMatch {
    distance: u32,
    remaining: u32,
}

/// Block state of a streaming decode.
#[derive(Debug, Clone, Copy)]
enum BlockState {
    /// A block header is expected next.
    Header,
    /// A stored block has `remaining` bytes left.
    Stored { remaining: u32, final_block: bool },
    /// A Huffman block is decoding tokens.
    Tokens { final_block: bool, pending: Option<PendingMatch> },
    /// The last block finished and the stream is complete.
    Done,
}

/// Bit cursor over an absolute-indexed input queue.
struct StreamBits<'q> {
    queue: &'q InputQueue,
    /// Absolute bit position of the next bit to read.
    bit_pos: u64,
    /// Bit position of the stream start, used for reported positions.
    origin_bit: u64,
    cache_start: u64,
    cache_len: u64,
    cache: [u8; BIT_CACHE_BYTES as usize],
}

impl<'q> StreamBits<'q> {
    fn new(queue: &'q InputQueue, bit_pos: u64, origin_bit: u64) -> Self {
        Self { queue, bit_pos, origin_bit, cache_start: 0, cache_len: 0, cache: [0; BIT_CACHE_BYTES as usize] }
    }

    fn byte_offset(&self) -> usize {
        ((self.bit_pos - self.origin_bit) / 8) as usize
    }

    fn align_to_byte(&mut self) {
        self.bit_pos = (self.bit_pos + 7) & !7;
    }

    fn byte_index(&self) -> u64 {
        self.bit_pos / 8
    }

    fn load(&mut self, byte: u64) -> Result<u8, CodecError> {
        if byte < self.cache_start || byte >= self.cache_start + self.cache_len {
            let end = self.queue.end();
            if byte >= end {
                return Err(exhausted(self.byte_offset(), (byte + 1 - end) as usize));
            }
            let count = (end - byte).min(BIT_CACHE_BYTES) as usize;
            for index in 0..count {
                self.cache[index] = self.queue.get(byte + index as u64).unwrap_or(0);
            }
            self.cache_start = byte;
            self.cache_len = count as u64;
        }
        Ok(self.cache[(byte - self.cache_start) as usize])
    }
}

impl BitSource for StreamBits<'_> {
    fn read_bits(&mut self, count: u8) -> Result<u32, CodecError> {
        if count == 0 {
            return Ok(0);
        }
        if count > MAX_READ_BITS {
            return Err(CodecError::InvalidBitPattern { offset: self.byte_offset(), width: count });
        }
        let end = self.bit_pos + u64::from(count);
        let last_byte = (end - 1) / 8;
        if last_byte >= self.queue.end() {
            return Err(exhausted(self.byte_offset(), (last_byte + 1 - self.queue.end()) as usize));
        }

        let mut value = 0u32;
        let mut position = self.bit_pos;
        let mut remaining = count;
        let mut shift = 0u32;
        while remaining > 0 {
            let byte_index = position / 8;
            let byte = self.load(byte_index)?;
            let in_byte = (position % 8) as u8;
            let take = remaining.min(8 - in_byte);
            let mask = if take >= 8 { 0xFF } else { (1u8 << take) - 1 };
            let chunk = (byte >> in_byte) & mask;
            value |= u32::from(chunk) << shift;
            shift += u32::from(take);
            position += u64::from(take);
            remaining -= take;
        }
        self.bit_pos = end;
        Ok(value)
    }

    fn position(&self) -> BitPosition {
        let total = self.bit_pos - self.origin_bit;
        BitPosition::new(total / 8, (total % 8) as u8)
    }
}

/// Runs `f` and rolls the cursor back when it runs out of input.
fn attempt<T>(
    bits: &mut StreamBits<'_>,
    f: impl FnOnce(&mut StreamBits<'_>) -> Result<T, CodecError>,
) -> Result<Option<T>, CodecError> {
    let mark = bits.bit_pos;
    match f(bits) {
        Ok(value) => Ok(Some(value)),
        Err(CodecError::InputExhausted { .. }) => {
            bits.bit_pos = mark;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Deflate decoder that keeps its position across bounded reads.
pub(crate) struct DeflateDecoderCore {
    start_bit: u64,
    bit_pos: u64,
    window: SlidingWindow,
    state: BlockState,
    literal: Option<HuffmanTable>,
    distance: Option<HuffmanTable>,
    produced: u64,
    block_count: u64,
    steps: u64,
    limits: CodecLimits,
    finished: bool,
}

impl DeflateDecoderCore {
    /// Creates a decoder whose stream starts at `start_bit`.
    pub(crate) fn new(start_bit: u64, limits: CodecLimits) -> Self {
        Self {
            start_bit,
            bit_pos: start_bit,
            window: SlidingWindow::new(),
            state: BlockState::Header,
            literal: None,
            distance: None,
            produced: 0,
            block_count: 0,
            steps: 0,
            limits,
            finished: false,
        }
    }

    /// Absolute bit position of the next unread bit.
    pub(crate) fn bit_pos(&self) -> u64 {
        self.bit_pos
    }

    /// Bytes produced by this stream so far.
    pub(crate) fn produced(&self) -> u64 {
        self.produced
    }

    /// Absolute byte offset one past the last byte the stream touched.
    pub(crate) fn consumed_byte(&self) -> u64 {
        (self.bit_pos + 7) / 8
    }

    fn check_output(&self, produced: u64) -> Result<(), CodecError> {
        let limit = self.limits.output_limit();
        if produced >= limit { Err(CodecError::OutputLimitExceeded { produced, limit }) } else { Ok(()) }
    }

    fn check_expansion(&self, bit_pos: u64) -> Result<(), CodecError> {
        let input = (bit_pos / 8).saturating_sub(self.start_bit / 8);
        if !self.limits.check_expansion(input, self.produced) {
            return Err(CodecError::ExpansionRatioExceeded {
                input,
                output: self.produced,
                limit: self.limits.max_expansion_ratio.unwrap_or(crate::error::Ratio::new(1, 1)),
            });
        }
        Ok(())
    }

    fn finish_block(&mut self, bits: &StreamBits<'_>, final_block: bool) -> Result<(), CodecError> {
        self.check_expansion(bits.bit_pos)?;
        self.state = if final_block { BlockState::Done } else { BlockState::Header };
        Ok(())
    }

    /// Decodes into `out` until it is full, the input runs short, or the stream ends.
    pub(crate) fn read(&mut self, queue: &InputQueue, out: &mut [u8]) -> Result<StreamChunk, CodecError> {
        if self.finished {
            return Ok(StreamChunk::Finished);
        }
        if out.is_empty() {
            return Ok(StreamChunk::Produced(0));
        }

        let mut bits = StreamBits::new(queue, self.bit_pos, self.start_bit);
        let mut written = 0usize;
        let result = self.pump(&mut bits, queue, out, &mut written);
        self.bit_pos = bits.bit_pos;
        match result {
            Ok(()) if written > 0 => Ok(StreamChunk::Produced(written)),
            Ok(()) if self.finished => Ok(StreamChunk::Finished),
            Ok(()) => Ok(StreamChunk::NeedInput),
            Err(error) => Err(error),
        }
    }

    fn pump(
        &mut self,
        bits: &mut StreamBits<'_>,
        queue: &InputQueue,
        out: &mut [u8],
        written: &mut usize,
    ) -> Result<(), CodecError> {
        loop {
            if *written >= out.len() {
                return Ok(());
            }
            match self.state {
                BlockState::Done => {
                    self.finished = true;
                    return Ok(());
                }
                BlockState::Header => {
                    if self.block_count_limit_reached() {
                        let limit = self.limits.max_block_count.unwrap_or(u64::MAX);
                        return Err(CodecError::OutputLimitExceeded { produced: self.produced, limit });
                    }
                    let Some((final_block, header)) = attempt(bits, |bits| read_block_header(bits, self.limits))?
                    else {
                        return Ok(());
                    };
                    self.block_count += 1;
                    self.steps = 0;
                    match header {
                        BlockHeader::Stored { remaining } => {
                            bits.align_to_byte();
                            self.literal = None;
                            self.distance = None;
                            self.state = BlockState::Stored { remaining, final_block };
                        }
                        BlockHeader::Fixed => {
                            self.literal = Some(fixed_literal_table());
                            self.distance = Some(fixed_distance_table());
                            self.state = BlockState::Tokens { final_block, pending: None };
                        }
                        BlockHeader::Dynamic { literal, distance } => {
                            self.literal = Some(literal);
                            self.distance = Some(distance);
                            self.state = BlockState::Tokens { final_block, pending: None };
                        }
                    }
                }
                BlockState::Stored { remaining, final_block } => {
                    if remaining == 0 {
                        self.finish_block(bits, final_block)?;
                        continue;
                    }
                    let available = queue.available(bits.byte_index());
                    let capacity = out.len() - *written;
                    let take = (remaining as usize).min(capacity).min(available);
                    if take == 0 {
                        return Ok(());
                    }
                    let bytes = queue.slice_from(bits.byte_index());
                    self.check_output(self.produced + take as u64)?;
                    for index in 0..take {
                        out[*written + index] = self.window.push_byte(bytes[index]);
                    }
                    *written += take;
                    self.produced += take as u64;
                    bits.bit_pos += take as u64 * 8;
                    let left = remaining - take as u32;
                    self.state = BlockState::Stored { remaining: left, final_block };
                    if left == 0 {
                        self.finish_block(bits, final_block)?;
                    }
                }
                BlockState::Tokens { final_block, pending } => {
                    if let Some(pending_match) = pending {
                        let capacity = out.len() - *written;
                        let take = (pending_match.remaining as usize).min(capacity);
                        self.check_output(self.produced + take as u64)?;
                        for index in 0..take {
                            out[*written + index] = self.window.back_byte(pending_match.distance);
                        }
                        *written += take;
                        self.produced += take as u64;
                        if pending_match.remaining > take as u32 {
                            self.state = BlockState::Tokens {
                                final_block,
                                pending: Some(PendingMatch {
                                    distance: pending_match.distance,
                                    remaining: pending_match.remaining - take as u32,
                                }),
                            };
                            return Ok(());
                        }
                        self.state = BlockState::Tokens { final_block, pending: None };
                        continue;
                    }

                    let literal = self.literal.as_ref().expect("literal table for token block");
                    let distance = self.distance.as_ref().expect("distance table for token block");
                    let steps_limit = self.limits.max_steps;
                    if let Some(max_steps) = steps_limit {
                        if self.steps >= max_steps {
                            return Err(CodecError::OutputLimitExceeded { produced: self.produced, limit: max_steps });
                        }
                    }
                    self.steps += 1;
                    let Some(token) = attempt(bits, |bits| decode_token(bits, literal, distance, self.limits))?
                    else {
                        return Ok(());
                    };
                    match token {
                        Token::Literal(byte) => {
                            self.check_output(self.produced)?;
                            out[*written] = self.window.push_byte(byte);
                            *written += 1;
                            self.produced += 1;
                        }
                        Token::Match { length, distance } => {
                            if let Some(max_distance) = self.limits.max_distance {
                                if distance > max_distance {
                                    return Err(CodecError::DistanceOutOfWindow {
                                        position: bits.position(),
                                        distance,
                                        window: max_distance,
                                    });
                                }
                            }
                            if !self.window.can_reference(distance) {
                                return Err(CodecError::InvalidBackReference {
                                    position: bits.position(),
                                    distance,
                                    available: self.window.len() as u32,
                                });
                            }
                            self.check_output(self.produced + u64::from(length))?;
                            let capacity = out.len() - *written;
                            let take = (length as usize).min(capacity);
                            for index in 0..take {
                                out[*written + index] = self.window.back_byte(distance);
                            }
                            *written += take;
                            self.produced += take as u64;
                            if length > take as u32 {
                                self.state = BlockState::Tokens {
                                    final_block,
                                    pending: Some(PendingMatch { distance, remaining: length - take as u32 }),
                                };
                            }
                        }
                        Token::EndOfBlock => {
                            self.finish_block(bits, final_block)?;
                        }
                    }
                }
            }
        }
    }

    fn block_count_limit_reached(&self) -> bool {
        self.limits.max_block_count.is_some_and(|max| self.block_count >= max)
    }
}

/// Header form decoded before the block payload starts.
enum BlockHeader {
    Stored { remaining: u32 },
    Fixed,
    Dynamic { literal: HuffmanTable, distance: HuffmanTable },
}

fn read_block_header(bits: &mut StreamBits<'_>, limits: CodecLimits) -> Result<(bool, BlockHeader), CodecError> {
    let final_block = bits.read_bits(1)? != 0;
    let block_type = bits.read_bits(2)?;
    match block_type {
        0 => {
            bits.align_to_byte();
            let len = bits.read_bits(16)? as u16;
            let nlen = bits.read_bits(16)? as u16;
            if len != (nlen ^ 0xFFFF) {
                return Err(CodecError::InvalidBlockHeader { position: bits.position() });
            }
            Ok((final_block, BlockHeader::Stored { remaining: u32::from(len) }))
        }
        1 => Ok((final_block, BlockHeader::Fixed)),
        2 => {
            let (literal, distance) = decode_dynamic_tables(bits, limits)?;
            Ok((final_block, BlockHeader::Dynamic { literal, distance }))
        }
        other => Err(CodecError::UnsupportedFeature { feature: crate::error::CodecFeature::BlockType(other as u8) }),
    }
}

fn decode_dynamic_tables(bits: &mut StreamBits<'_>, limits: CodecLimits) -> Result<(HuffmanTable, HuffmanTable), CodecError> {
    let hlit = bits.read_bits(5)? as usize + 257;
    let hdist = bits.read_bits(5)? as usize + 1;
    let hclen = bits.read_bits(4)? as usize + 4;

    if let Some(max_symbols) = limits.max_huffman_symbols {
        if (hlit + hdist) as u64 > max_symbols {
            return Err(CodecError::InvalidHuffmanTree { position: bits.position() });
        }
    }

    let mut code_lengths = vec![0u8; 19];
    for index in 0..hclen {
        code_lengths[CODE_LENGTH_ORDER[index] as usize] = bits.read_bits(3)? as u8;
    }
    let code_table = build_huffman(&code_lengths, bits.position())?;

    let mut lengths = vec![0u8; hlit + hdist];
    let mut index = 0usize;
    while index < lengths.len() {
        let symbol = code_table.decode(bits)?;
        match symbol {
            0..=15 => {
                lengths[index] = symbol as u8;
                index += 1;
            }
            16 => {
                let repeat = bits.read_bits(2)? as usize + 3;
                if index == 0 || index + repeat > lengths.len() {
                    return Err(CodecError::InvalidHuffmanTree { position: bits.position() });
                }
                let previous = lengths[index - 1];
                lengths[index..index + repeat].fill(previous);
                index += repeat;
            }
            17 => {
                let repeat = bits.read_bits(3)? as usize + 3;
                if index + repeat > lengths.len() {
                    return Err(CodecError::InvalidHuffmanTree { position: bits.position() });
                }
                index += repeat;
            }
            18 => {
                let repeat = bits.read_bits(7)? as usize + 11;
                if index + repeat > lengths.len() {
                    return Err(CodecError::InvalidHuffmanTree { position: bits.position() });
                }
                index += repeat;
            }
            _ => return Err(CodecError::HuffmanSymbolOutOfRange { position: bits.position(), symbol }),
        }
    }

    let position = bits.position();
    let literal = build_huffman(&lengths[..hlit], position)?;
    let distance = if lengths[hlit..].iter().all(|&len| len == 0) {
        HuffmanTable::empty()
    }
    else {
        build_huffman(&lengths[hlit..], position)?
    };
    Ok((literal, distance))
}

fn build_huffman(lengths: &[u8], position: BitPosition) -> Result<HuffmanTable, CodecError> {
    HuffmanTable::from_lengths(lengths).map_err(|_| CodecError::InvalidHuffmanTree { position })
}

fn decode_token(
    bits: &mut StreamBits<'_>,
    literal: &HuffmanTable,
    distance_table: &HuffmanTable,
    limits: CodecLimits,
) -> Result<Token, CodecError> {
    let symbol = literal.decode(bits)?;
    if symbol < 256 {
        return Ok(Token::Literal(symbol as u8));
    }
    if symbol == 256 {
        return Ok(Token::EndOfBlock);
    }

    let length_index = (symbol - 257) as usize;
    if length_index >= LENGTH_BASE.len() {
        return Err(CodecError::HuffmanSymbolOutOfRange { position: bits.position(), symbol });
    }
    let extra_bits = LENGTH_EXTRA[length_index];
    let length = u32::from(LENGTH_BASE[length_index]) + if extra_bits > 0 { bits.read_bits(extra_bits)? } else { 0 };

    let distance_symbol = distance_table.decode(bits)?;
    let distance_index = distance_symbol as usize;
    if distance_index >= DIST_BASE.len() {
        return Err(CodecError::HuffmanSymbolOutOfRange { position: bits.position(), symbol: distance_symbol });
    }
    let distance_extra = DIST_EXTRA[distance_index];
    let distance = u32::from(DIST_BASE[distance_index]) + if distance_extra > 0 { bits.read_bits(distance_extra)? } else { 0 };

    if let Some(max_distance) = limits.max_distance {
        if distance > max_distance {
            return Err(CodecError::DistanceOutOfWindow { position: bits.position(), distance, window: max_distance });
        }
    }
    Ok(Token::Match { length, distance })
}

/// Pull-style raw Deflate decoder fed by bounded chunks.
///
/// `push` accepts another input chunk and `read` writes at most the caller's
/// buffer size. Dropping the decoder releases everything it holds, which is how
/// a downstream consumer cancels a transfer.
pub struct DeflateStreamDecoder {
    queue: InputQueue,
    core: DeflateDecoderCore,
    limits: CodecLimits,
    pushed: u64,
    state: StreamState,
}

impl DeflateStreamDecoder {
    /// Creates an empty decoder.
    pub fn new(limits: CodecLimits) -> Self {
        Self {
            queue: InputQueue::new(),
            core: DeflateDecoderCore::new(0, limits),
            limits,
            pushed: 0,
            state: StreamState::Created,
        }
    }

    /// Returns the lifecycle state of this decoder.
    #[inline]
    pub const fn state(&self) -> StreamState {
        self.state
    }

    /// How many input bytes have been accepted so far.
    #[inline]
    pub fn input_bytes(&self) -> u64 {
        self.pushed
    }

    /// How many decoded bytes have been produced so far.
    #[inline]
    pub fn output_bytes(&self) -> u64 {
        self.core.produced()
    }

    /// Buffers another input chunk.
    ///
    /// The chunk is kept only when the total accepted input stays inside
    /// `max_input_bytes`, so an oversized stream is rejected before it can
    /// exhaust memory. A rejected chunk leaves the decoder unchanged.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), CodecError> {
        match self.state {
            StreamState::Complete | StreamState::Failed => {
                return Err(CodecError::InvalidStreamState { state: self.state });
            }
            StreamState::Created | StreamState::Receiving => {}
        }
        let limit = self.limits.max_input_bytes.unwrap_or(u64::MAX);
        let offered = chunk.len() as u64;
        let total =
            self.pushed.checked_add(offered).ok_or(CodecError::InputLimitExceeded { buffered: self.pushed, offered, limit })?;
        if total > limit {
            return Err(CodecError::InputLimitExceeded { buffered: self.pushed, offered, limit });
        }
        self.queue.push(chunk);
        self.pushed = total;
        self.state = StreamState::Receiving;
        Ok(())
    }

    /// Decodes at most `out.len()` bytes and reports why it stopped.
    ///
    /// A success that filled part of the buffer leaves the remaining input live
    /// for the next call. `NeedInput` means no byte could be committed and the
    /// caller may push more. `Finished` is returned for every call after the
    /// final block completed.
    pub fn read(&mut self, out: &mut [u8]) -> Result<StreamChunk, CodecError> {
        match self.state {
            StreamState::Complete => return Ok(StreamChunk::Finished),
            StreamState::Failed => return Err(CodecError::InvalidStreamState { state: self.state }),
            StreamState::Created | StreamState::Receiving => {}
        }
        let result = self.core.read(&self.queue, out);
        match result {
            Ok(chunk) => {
                self.queue.discard_before(self.core.bit_pos() / 8);
                match chunk {
                    StreamChunk::Finished => self.state = StreamState::Complete,
                    StreamChunk::Produced(_) if self.state == StreamState::Created => self.state = StreamState::Receiving,
                    StreamChunk::Produced(_) | StreamChunk::NeedInput => {}
                }
                Ok(chunk)
            }
            Err(error) => {
                self.state = StreamState::Failed;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression::deflate::{Deflate, DeflateLevel};

    fn decode_stream(input: &[u8], push: usize, read: usize) -> Vec<u8> {
        decode_stream_with(input, push, read, CodecLimits::new())
    }

    fn decode_stream_with(input: &[u8], push: usize, read: usize, limits: CodecLimits) -> Vec<u8> {
        let mut decoder = DeflateStreamDecoder::new(limits);
        let mut output = Vec::new();
        let mut offset = 0usize;
        let mut buffer = vec![0u8; read];
        loop {
            match decoder.read(&mut buffer).expect("stream decode") {
                StreamChunk::Produced(count) => output.extend_from_slice(&buffer[..count]),
                StreamChunk::Finished => return output,
                StreamChunk::NeedInput => {
                    assert!(offset < input.len(), "decoder requested input after the stream ended");
                    let end = (offset + push).min(input.len());
                    decoder.push(&input[offset..end]).expect("push");
                    offset = end;
                }
            }
        }
    }

    fn sample() -> Vec<u8> {
        let mut data = Vec::new();
        for index in 0..4096u32 {
            data.extend_from_slice(&(index % 97).to_le_bytes());
            if index % 5 == 0 {
                data.extend_from_slice(b"acorn-stream-deflate");
            }
        }
        data
    }

    #[test]
    fn chunked_input_matches_one_shot_for_every_level() {
        let data = sample();
        let expected =
            Deflate::decode(&Deflate::encode(&data, DeflateLevel::Default), CodecLimits::new()).expect("one-shot decode").value;
        assert_eq!(expected, data);
        for level in [DeflateLevel::Store, DeflateLevel::Fast, DeflateLevel::Default] {
            let encoded = Deflate::encode(&data, level);
            for push in [1usize, 3, 17, 4096] {
                for read in [1usize, 7, 64, 1024] {
                    assert_eq!(decode_stream(&encoded, push, read), data, "level {level:?} push {push} read {read}");
                }
            }
        }
    }

    #[test]
    fn output_limit_is_enforced_while_streaming() {
        let data = vec![0u8; 1024];
        let encoded = Deflate::encode(&data, DeflateLevel::Default);
        let limits = CodecLimits::new().with_max_output_bytes(16);
        let mut decoder = DeflateStreamDecoder::new(limits);
        decoder.push(&encoded).expect("push");
        let mut buffer = vec![0u8; 32];
        let mut produced = 0usize;
        loop {
            match decoder.read(&mut buffer) {
                Ok(StreamChunk::Produced(count)) => {
                    produced += count;
                    assert!(produced <= 16);
                }
                Ok(StreamChunk::NeedInput) => panic!("all input was supplied"),
                Ok(StreamChunk::Finished) => break,
                Err(CodecError::OutputLimitExceeded { limit, .. }) => {
                    assert_eq!(limit, 16);
                    break;
                }
                Err(other) => panic!("unexpected error: {other}"),
            }
        }
    }

    #[test]
    fn truncated_input_never_reports_finished() {
        let data = sample();
        let encoded = Deflate::encode(&data, DeflateLevel::Default);
        let mut decoder = DeflateStreamDecoder::new(CodecLimits::new());
        decoder.push(&encoded[..encoded.len() / 2]).expect("push");
        let mut buffer = vec![0u8; 64];
        loop {
            match decoder.read(&mut buffer).expect("decode prefix") {
                StreamChunk::Produced(_) => continue,
                StreamChunk::NeedInput => break,
                StreamChunk::Finished => panic!("half a stream cannot finish"),
            }
        }
        assert_eq!(decoder.state(), StreamState::Receiving);
    }

    #[test]
    fn read_after_finish_is_idempotent_and_push_is_rejected() {
        let encoded = Deflate::encode(b"hello", DeflateLevel::Fast);
        let mut decoder = DeflateStreamDecoder::new(CodecLimits::new());
        decoder.push(&encoded).expect("push");
        let mut buffer = vec![0u8; 32];
        let mut output = Vec::new();
        loop {
            match decoder.read(&mut buffer).expect("decode") {
                StreamChunk::Produced(count) => output.extend_from_slice(&buffer[..count]),
                StreamChunk::NeedInput => panic!("all input supplied"),
                StreamChunk::Finished => break,
            }
        }
        assert_eq!(output, b"hello");
        assert_eq!(decoder.read(&mut buffer).expect("second finish"), StreamChunk::Finished);
        assert!(matches!(decoder.push(b"x"), Err(CodecError::InvalidStreamState { .. })));
    }
}
