//! Incremental byte input for resumable decoders.
//!
//! The queue keeps bytes by position instead of shifting a cursor buffer, so a
//! decoder can express "give me byte 1234" without owning the whole stream.
//! Consumed prefixes are dropped once they fall behind every live cursor, which
//! keeps peak memory proportional to the caller's push chunk size rather than
//! to the archive size.

use crate::error::CodecError;

/// Bytes consumed before the buffer is compacted.
const COMPACT_THRESHOLD: usize = 64 * 1024;

/// Growable input addressed by absolute byte index.
#[derive(Debug, Default)]
pub(crate) struct InputQueue {
    /// Absolute byte index of `bytes[0]`.
    base: u64,
    /// Offset of the first byte that has not been discarded.
    start: usize,
    /// Backing storage; bytes before `start` are dead.
    bytes: Vec<u8>,
}

impl InputQueue {
    /// Creates an empty queue.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Appends a caller-supplied chunk.
    pub(crate) fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend_from_slice(chunk);
    }

    /// Returns the absolute index one past the last buffered byte.
    pub(crate) fn end(&self) -> u64 {
        self.base + self.bytes.len() as u64
    }

    /// Reads one byte at an absolute index.
    pub(crate) fn get(&self, index: u64) -> Option<u8> {
        let offset = usize::try_from(index.checked_sub(self.base)?).ok()?;
        if offset < self.start {
            return None;
        }
        self.bytes.get(offset).copied()
    }

    /// Returns the buffered bytes starting at an absolute index.
    pub(crate) fn slice_from(&self, index: u64) -> &[u8] {
        let offset = usize::try_from(index.saturating_sub(self.base)).unwrap_or(usize::MAX);
        let from = offset.max(self.start).min(self.bytes.len());
        &self.bytes[from..]
    }

    /// Drops every byte before `index`.
    ///
    /// Callers pass the lowest position any cursor may still read, so a decoder
    /// that restored its bit cursor never loses bytes it may need again.
    pub(crate) fn discard_before(&mut self, index: u64) {
        let offset = usize::try_from(index.saturating_sub(self.base)).unwrap_or(usize::MAX);
        let target = offset.min(self.bytes.len());
        if target <= self.start {
            return;
        }
        self.start = target;
        if self.start >= COMPACT_THRESHOLD {
            self.bytes.drain(..self.start);
            self.base += self.start as u64;
            self.start = 0;
        }
    }

    /// Number of live bytes buffered at or after `index`.
    pub(crate) fn available(&self, index: u64) -> usize {
        let live_start = self.base + self.start as u64;
        self.end().saturating_sub(index.max(live_start)) as usize
    }
}

/// Reports a shortage as `InputExhausted` so a streaming decoder can resume.
pub(crate) fn exhausted(offset: usize, needed: usize) -> CodecError {
    CodecError::InputExhausted { offset, needed }
}
