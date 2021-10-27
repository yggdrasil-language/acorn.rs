//! Incremental gzip envelope (RFC 1952).
//!
//! [`GzipStreamDecoder`] parses the member header, delegates the bitstream to
//! the resumable Deflate decoder, and validates the CRC32 and `ISIZE` trailer as
//! it becomes available. Only the header scan and the bit cursor are kept, so
//! the caller controls both the input chunk size and the output buffer size.
//!
//! [`GzipStreamEncoder`] mirrors that shape: it emits the fixed member header,
//! one Deflate block per fed chunk, then the CRC32 and `ISIZE` trailer.

use crate::{
    compression::{
        checksum::Crc32,
        deflate::{DeflateLevel, DeflateStreamEncoder, stream::DeflateDecoderCore},
        gzip::MEMBER_HEADER,
        input::InputQueue,
        stream::StreamChunk,
    },
    error::{ChecksumAlgorithm, CodecError, CodecFeature, EnvelopeKind},
    facts::{ChecksumFacts, StreamState},
    limits::CodecLimits,
};

/// Length of the fixed gzip member header.
const HEADER_LEN: u64 = 10;
/// Length of the CRC32 plus `ISIZE` trailer.
const TRAILER_LEN: u64 = 8;

const FLAG_EXTRA: u8 = 0x04;
const FLAG_NAME: u8 = 0x08;
const FLAG_COMMENT: u8 = 0x10;
const FLAG_HEADER_CRC: u8 = 0x02;

/// Scans the member header and returns the Deflate start offset.
///
/// Returns `None` when the buffered bytes do not yet reach the next field, so
/// the caller can push more. Header bytes stay at absolute position zero until
/// the scan succeeds, which is why no separate header state is needed.
fn parse_header(queue: &InputQueue) -> Result<Option<u64>, CodecError> {
    if queue.end() < HEADER_LEN {
        return Ok(None);
    }
    let byte = |index: u64| queue.get(index).unwrap_or(0);

    if byte(0) != 0x1F || byte(1) != 0x8B {
        return Err(CodecError::InvalidEnvelope { offset: 0, kind: EnvelopeKind::Gzip });
    }
    if byte(2) != 8 {
        return Err(CodecError::UnsupportedFeature { feature: CodecFeature::CompressionMethod(byte(2)) });
    }

    let flags = byte(3);
    let mut position = HEADER_LEN;

    if flags & FLAG_EXTRA != 0 {
        if queue.end() < position + 2 {
            return Ok(None);
        }
        let extra_len = u64::from(u16::from_le_bytes([byte(position), byte(position + 1)]));
        position += 2;
        if queue.end() < position + extra_len {
            return Ok(None);
        }
        position += extra_len;
    }
    if flags & FLAG_NAME != 0 {
        let Some(next) = skip_zero_terminated(queue, position)?
        else {
            return Ok(None);
        };
        position = next;
    }
    if flags & FLAG_COMMENT != 0 {
        let Some(next) = skip_zero_terminated(queue, position)?
        else {
            return Ok(None);
        };
        position = next;
    }
    if flags & FLAG_HEADER_CRC != 0 {
        if queue.end() < position + 2 {
            return Ok(None);
        }
        let expected = u16::from_le_bytes([byte(position), byte(position + 1)]);
        let mut header_crc = Crc32::new();
        header_crc.update(&queue.slice_from(0)[..position as usize]);
        let actual = header_crc.value() as u16;
        if expected != actual {
            return Err(CodecError::ChecksumMismatch {
                algorithm: ChecksumAlgorithm::Crc32,
                expected: u64::from(expected),
                actual: u64::from(actual),
            });
        }
        position += 2;
    }

    if queue.end() <= position {
        return Ok(None);
    }
    Ok(Some(position))
}

/// Advances past a zero-terminated header field.
///
/// `None` means the terminator is not buffered yet.
fn skip_zero_terminated(queue: &InputQueue, offset: u64) -> Result<Option<u64>, CodecError> {
    let end = queue.end();
    let mut position = offset;
    while position < end {
        if queue.get(position) == Some(0) {
            return Ok(Some(position + 1));
        }
        position += 1;
    }
    Ok(None)
}

/// Private state of a gzip streaming decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GzipState {
    /// The member header is still being scanned.
    Header,
    /// The Deflate bitstream is decoding.
    Deflate,
    /// The Deflate stream ended; the trailer starts at `start`.
    Trailer { start: u64 },
    /// The member finished and the checksum was recorded.
    Done,
}

/// Pull-style gzip member decoder fed by bounded chunks.
///
/// Dropping the decoder releases the bit cursor, window, and buffered input.
/// A downstream copy loop therefore cancels by simply abandoning the reader.
pub struct GzipStreamDecoder {
    queue: InputQueue,
    core: Option<DeflateDecoderCore>,
    limits: CodecLimits,
    pushed: u64,
    produced: u64,
    crc: Crc32,
    state: GzipState,
    public: StreamState,
    checksum: Option<ChecksumFacts>,
    trailing: u64,
}

impl GzipStreamDecoder {
    /// Creates an empty decoder.
    pub fn new(limits: CodecLimits) -> Self {
        Self {
            queue: InputQueue::new(),
            core: None,
            limits,
            pushed: 0,
            produced: 0,
            crc: Crc32::new(),
            state: GzipState::Header,
            public: StreamState::Created,
            checksum: None,
            trailing: 0,
        }
    }

    /// Returns the lifecycle state.
    #[inline]
    pub const fn state(&self) -> StreamState {
        self.public
    }

    /// How many input bytes have been accepted so far.
    #[inline]
    pub fn input_bytes(&self) -> u64 {
        self.pushed
    }

    /// How many decoded bytes have been produced so far.
    #[inline]
    pub fn output_bytes(&self) -> u64 {
        self.produced
    }

    /// Checksum result once the member completed.
    #[inline]
    pub const fn checksum(&self) -> Option<ChecksumFacts> {
        self.checksum
    }

    /// Bytes buffered after the member trailer, valid once the member completed.
    #[inline]
    pub const fn trailing_input(&self) -> u64 {
        self.trailing
    }

    /// Buffers another input chunk.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), CodecError> {
        match self.public {
            StreamState::Complete | StreamState::Failed => {
                return Err(CodecError::InvalidStreamState { state: self.public });
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
        self.public = StreamState::Receiving;
        Ok(())
    }

    /// Decodes at most `out.len()` bytes and reports why it stopped.
    pub fn read(&mut self, out: &mut [u8]) -> Result<StreamChunk, CodecError> {
        match self.public {
            StreamState::Complete => return Ok(StreamChunk::Finished),
            StreamState::Failed => return Err(CodecError::InvalidStreamState { state: self.public }),
            StreamState::Created | StreamState::Receiving => {}
        }
        match self.read_inner(out) {
            Ok(chunk) => Ok(chunk),
            Err(error) => {
                self.public = StreamState::Failed;
                Err(error)
            }
        }
    }

    fn read_inner(&mut self, out: &mut [u8]) -> Result<StreamChunk, CodecError> {
        if out.is_empty() {
            return Ok(StreamChunk::Produced(0));
        }
        loop {
            match self.state {
                GzipState::Header => match parse_header(&self.queue)? {
                    Some(start) => {
                        self.queue.discard_before(start);
                        self.core = Some(DeflateDecoderCore::new(start * 8, self.limits));
                        self.state = GzipState::Deflate;
                    }
                    None => {
                        self.public = StreamState::Receiving;
                        return Ok(StreamChunk::NeedInput);
                    }
                },
                GzipState::Deflate => {
                    let core = self.core.as_mut().expect("deflate core while decoding");
                    match core.read(&self.queue, out)? {
                        StreamChunk::Produced(count) => {
                            self.crc.update(&out[..count]);
                            self.produced += count as u64;
                            let consumed = core.bit_pos() / 8;
                            self.queue.discard_before(consumed);
                            self.public = StreamState::Receiving;
                            return Ok(StreamChunk::Produced(count));
                        }
                        StreamChunk::NeedInput => {
                            let consumed = core.bit_pos() / 8;
                            self.queue.discard_before(consumed);
                            self.public = StreamState::Receiving;
                            return Ok(StreamChunk::NeedInput);
                        }
                        StreamChunk::Finished => {
                            let start = core.consumed_byte();
                            self.queue.discard_before(start);
                            self.state = GzipState::Trailer { start };
                        }
                    }
                }
                GzipState::Trailer { start } => {
                    if self.queue.end() < start + TRAILER_LEN {
                        self.public = StreamState::Receiving;
                        return Ok(StreamChunk::NeedInput);
                    }
                    let byte = |index: u64| self.queue.get(index).unwrap_or(0);
                    let expected_crc = u32::from_le_bytes([byte(start), byte(start + 1), byte(start + 2), byte(start + 3)]);
                    let actual_crc = self.crc.value();
                    if expected_crc != actual_crc {
                        return Err(CodecError::ChecksumMismatch {
                            algorithm: ChecksumAlgorithm::Crc32,
                            expected: u64::from(expected_crc),
                            actual: u64::from(actual_crc),
                        });
                    }
                    let expected_size =
                        u32::from_le_bytes([byte(start + 4), byte(start + 5), byte(start + 6), byte(start + 7)]);
                    if expected_size != self.produced as u32 {
                        return Err(CodecError::InvalidEnvelope { offset: start + 4, kind: EnvelopeKind::Gzip });
                    }
                    self.trailing = self.queue.end() - (start + TRAILER_LEN);
                    self.checksum = Some(ChecksumFacts {
                        algorithm: ChecksumAlgorithm::Crc32,
                        value: u64::from(actual_crc),
                        verified: true,
                    });
                    self.state = GzipState::Done;
                    self.public = StreamState::Complete;
                    self.queue.discard_before(self.queue.end());
                    return Ok(StreamChunk::Finished);
                }
                GzipState::Done => {
                    self.public = StreamState::Complete;
                    return Ok(StreamChunk::Finished);
                }
            }
        }
    }
}

/// Incremental gzip encoder.
///
/// Emits the fixed member header once, one Deflate block per fed chunk, then the
/// CRC32 and `ISIZE` trailer. Peak memory is bounded by the largest fed chunk.
#[derive(Debug)]
pub struct GzipStreamEncoder {
    deflate: DeflateStreamEncoder,
    crc: Crc32,
    size: u64,
    started: bool,
    finished: bool,
}

impl GzipStreamEncoder {
    /// Creates an encoder for `level`.
    pub fn new(level: DeflateLevel) -> Self {
        Self { deflate: DeflateStreamEncoder::new(level), crc: Crc32::new(), size: 0, started: false, finished: false }
    }

    /// Emits the member header, at most once.
    pub fn start(&mut self) -> Vec<u8> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        MEMBER_HEADER.to_vec()
    }

    /// How many input bytes have been fed so far.
    #[inline]
    pub const fn input_bytes(&self) -> u64 {
        self.size
    }

    /// Compresses one bounded chunk, prefixing the header on the first call.
    pub fn feed(&mut self, input: &[u8]) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        let mut output = self.start();
        self.crc.update(input);
        self.size += input.len() as u64;
        output.extend_from_slice(&self.deflate.feed(input));
        output
    }

    /// Emits the terminal Deflate block and trailer exactly once.
    pub fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut output = self.start();
        output.extend_from_slice(&self.deflate.finish());
        output.extend_from_slice(&self.crc.value().to_le_bytes());
        output.extend_from_slice(&(self.size as u32).to_le_bytes());
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression::{Gzip, StreamChunk};

    fn decode_stream(input: &[u8], push: usize, read: usize) -> Vec<u8> {
        let mut decoder = GzipStreamDecoder::new(CodecLimits::new());
        let mut output = Vec::new();
        let mut offset = 0usize;
        let mut buffer = vec![0u8; read];
        loop {
            match decoder.read(&mut buffer).expect("gzip stream decode") {
                StreamChunk::Produced(count) => output.extend_from_slice(&buffer[..count]),
                StreamChunk::Finished => {
                    assert!(decoder.checksum().is_some_and(|facts| facts.verified));
                    return output;
                }
                StreamChunk::NeedInput => {
                    assert!(offset < input.len(), "gzip decoder requested input after the stream ended");
                    let end = (offset + push).min(input.len());
                    decoder.push(&input[offset..end]).expect("push");
                    offset = end;
                }
            }
        }
    }

    fn sample() -> Vec<u8> {
        let mut data = Vec::new();
        for index in 0..2048u32 {
            data.extend_from_slice(&(index % 251).to_le_bytes());
            if index % 3 == 0 {
                data.extend_from_slice(b"gzip-stream-envelope");
            }
        }
        data
    }

    #[test]
    fn chunked_decode_matches_one_shot() {
        let data = sample();
        let encoded = Gzip::encode(&data, DeflateLevel::Default);
        for push in [1usize, 5, 64, 4096] {
            for read in [1usize, 9, 128, 2048] {
                assert_eq!(decode_stream(&encoded, push, read), data, "push {push} read {read}");
            }
        }
    }

    #[test]
    fn trailer_checksum_failure_is_reported() {
        let data = sample();
        let encoded = Gzip::encode(&data, DeflateLevel::Fast);
        let mut corrupted = encoded.clone();
        let trailer = corrupted.len() - 8;
        corrupted[trailer] ^= 0xFF;

        let mut decoder = GzipStreamDecoder::new(CodecLimits::new());
        decoder.push(&corrupted).expect("push");
        let mut buffer = vec![0u8; 512];
        loop {
            match decoder.read(&mut buffer) {
                Ok(StreamChunk::Produced(_)) => continue,
                Ok(StreamChunk::NeedInput) => panic!("all input supplied"),
                Ok(StreamChunk::Finished) => panic!("corrupted checksum must not finish"),
                Err(CodecError::ChecksumMismatch { algorithm, .. }) => {
                    assert_eq!(algorithm, ChecksumAlgorithm::Crc32);
                    break;
                }
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert_eq!(decoder.state(), StreamState::Failed);
    }

    #[test]
    fn truncated_stream_requests_more_input() {
        let encoded = Gzip::encode(&sample(), DeflateLevel::Default);
        let mut decoder = GzipStreamDecoder::new(CodecLimits::new());
        decoder.push(&encoded[..encoded.len() - 8]).expect("push");
        let mut buffer = vec![0u8; 256];
        let mut produced = 0usize;
        loop {
            match decoder.read(&mut buffer).expect("decode without trailer") {
                StreamChunk::Produced(count) => produced += count,
                StreamChunk::NeedInput => break,
                StreamChunk::Finished => panic!("trailer is missing"),
            }
        }
        assert!(produced > 0);
        assert_eq!(decoder.state(), StreamState::Receiving);
    }

    #[test]
    fn stream_encoder_round_trips_through_one_shot_decoder() {
        let data = sample();
        let mut encoder = GzipStreamEncoder::new(DeflateLevel::Default);
        let mut encoded = Vec::new();
        for chunk in data.chunks(333) {
            encoded.extend_from_slice(&encoder.feed(chunk));
        }
        encoded.extend_from_slice(&encoder.finish());
        assert!(encoder.finish().is_empty(), "finish is once-only");

        let facts = Gzip::decode(&encoded, CodecLimits::new()).expect("decode streamed gzip");
        assert_eq!(facts.value, data);
        assert!(facts.fully_consumed(encoded.len()));
    }
}
