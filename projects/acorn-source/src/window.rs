use crate::{
    SourceCapabilities,
    identity::SourceIdentity,
    read::{ReadOutcome, ReadResult, SourceRange},
    source::ByteSource,
};

/// A bounded view over another `ByteSource`.
#[derive(Debug, Clone)]
pub struct ByteWindow<S: ByteSource> {
    source: S,
    base: u64,
    range: SourceRange,
    complete: bool,
}

impl<S: ByteSource> ByteWindow<S> {
    /// Creates a window over `source`.
    pub fn new(source: S, base: u64, range: SourceRange) -> Result<Self, crate::RangeError> {
        range.end()?;
        Ok(Self { source, base, range, complete: false })
    }

    /// Marks the window as fully materialized.
    pub fn mark_complete(&mut self) {
        self.complete = true;
    }

    /// Returns whether the window is known to be complete.
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    /// Returns the window range relative to the outer source.
    pub const fn outer_range(&self) -> SourceRange {
        self.range
    }
}

impl<S: ByteSource> ByteSource for ByteWindow<S> {
    fn length(&self) -> Option<u64> {
        Some(self.range.len)
    }

    fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult {
        if offset >= self.range.len {
            return Ok(ReadOutcome::Eof);
        }
        let outer_offset = self.base + self.range.start + offset;
        let max_len = (self.range.len - offset) as usize;
        let bounded = destination.len().min(max_len);
        let mut scratch = vec![0u8; bounded];
        match self.source.read_at(outer_offset, &mut scratch)? {
            ReadOutcome::Complete => {
                destination[..bounded].copy_from_slice(&scratch);
                if bounded == destination.len() {
                    Ok(ReadOutcome::Complete)
                }
                else {
                    Ok(ReadOutcome::Partial { bytes_read: bounded as u64 })
                }
            }
            ReadOutcome::Partial { bytes_read } => {
                destination[..bytes_read as usize].copy_from_slice(&scratch[..bytes_read as usize]);
                Ok(ReadOutcome::Partial { bytes_read })
            }
            ReadOutcome::Eof => Ok(ReadOutcome::Eof),
            ReadOutcome::Unavailable { range, reason } => Ok(ReadOutcome::Unavailable { range, reason }),
        }
    }

    fn capabilities(&self) -> SourceCapabilities {
        let inner = self.source.capabilities();
        SourceCapabilities {
            random_access: inner.random_access,
            known_length: true,
            repeatable: inner.repeatable,
            concurrent_reads: inner.concurrent_reads,
            stable_identity: inner.stable_identity,
            decoded_views: inner.decoded_views,
        }
    }

    fn identity(&self) -> SourceIdentity {
        // A window is a different source than its parent. Two windows over the same
        // bytes must not claim one identity, so the outer interval becomes part of
        // the label while the content digest stays anchored to the parent bytes.
        let outer = self.source.identity();
        let start = self.base.saturating_add(self.range.start);
        let end = start.saturating_add(self.range.len);
        SourceIdentity {
            label: format!("{}[{}..{}]", outer.label, start, end),
            version: outer.version,
            content_digest: outer.content_digest,
        }
    }
}
