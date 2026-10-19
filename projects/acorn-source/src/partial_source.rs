use crate::{
    capabilities::SourceCapabilities,
    identity::SourceIdentity,
    read::{ReadOutcome, ReadResult, SourceRange},
    source::{ByteSource, ReadError},
};

/// A source that only exposes a set of known byte ranges.
#[derive(Debug, Clone)]
pub struct RangeMapSource {
    identity: SourceIdentity,
    segments: Vec<(u64, Vec<u8>)>,
    total_len: Option<u64>,
}

impl RangeMapSource {
    /// Creates a partial source with known segments.
    pub fn new(label: impl Into<String>, segments: Vec<(u64, Vec<u8>)>) -> Self {
        Self { identity: SourceIdentity::new(label), segments, total_len: None }
    }

    /// Declares the total length when known.
    pub fn with_total_length(mut self, total_len: u64) -> Self {
        self.total_len = Some(total_len);
        self
    }

    /// Supplies a byte range that was fetched after a missing-range report.
    ///
    /// Overlapping bytes must agree with what is already known. Two different values
    /// for one offset mean the underlying source changed, so the contradiction is
    /// reported instead of patching the stale snapshot. A range past a known total
    /// length is rejected rather than accepted as real content.
    pub fn provide_range(&mut self, start: u64, bytes: Vec<u8>) -> Result<(), ReadError> {
        if bytes.is_empty() {
            return Ok(());
        }
        let end = start
            .checked_add(bytes.len() as u64)
            .ok_or(ReadError::ConflictingRange { range: SourceRange { start, len: 0 } })?;
        if let Some(total) = self.total_len
            && end > total
        {
            return Err(ReadError::Unavailable {
                range: SourceRange { start, len: bytes.len() as u64 },
                reason: "supplied range extends past the known source length".to_string(),
            });
        }
        for (seg_start, seg_bytes) in &self.segments {
            let overlap_start = (*seg_start).max(start);
            let overlap_end = seg_start.saturating_add(seg_bytes.len() as u64).min(end);
            if overlap_end > overlap_start {
                let existing = &seg_bytes[(overlap_start - seg_start) as usize..(overlap_end - seg_start) as usize];
                let supplied = &bytes[(overlap_start - start) as usize..(overlap_end - start) as usize];
                if existing != supplied {
                    return Err(ReadError::ConflictingRange {
                        range: SourceRange { start: overlap_start, len: overlap_end - overlap_start },
                    });
                }
            }
        }
        self.segments.push((start, bytes));
        Ok(())
    }

    fn segment_for_offset(&self, offset: u64) -> Option<&[u8]> {
        self.segments.iter().find(|(start, bytes)| offset >= *start && offset < start.saturating_add(bytes.len() as u64)).map(
            |(start, bytes)| {
                let local = (offset - start) as usize;
                &bytes[local..]
            },
        )
    }
}

impl ByteSource for RangeMapSource {
    fn length(&self) -> Option<u64> {
        self.total_len
    }

    fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult {
        if let Some(total) = self.total_len
            && offset >= total
        {
            return Ok(ReadOutcome::Eof);
        }
        match self.segment_for_offset(offset) {
            Some(available) => {
                let to_copy = destination.len().min(available.len());
                destination[..to_copy].copy_from_slice(&available[..to_copy]);
                if to_copy == destination.len() {
                    Ok(ReadOutcome::Complete)
                }
                else {
                    Ok(ReadOutcome::Partial { bytes_read: to_copy as u64 })
                }
            }
            None => {
                let missing =
                    SourceRange::new(offset, destination.len() as u64).unwrap_or(SourceRange { start: offset, len: 0 });
                Ok(ReadOutcome::Unavailable { range: missing, reason: "byte range not yet provided".to_string() })
            }
        }
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::PARTIAL
    }

    /// Returns the label-based identity.
    ///
    /// A partial source has not materialized the bytes it does not hold, so its
    /// identity carries no content digest and snapshot reuse is refused rather than
    /// assumed from the label.
    fn identity(&self) -> SourceIdentity {
        self.identity.clone()
    }
}
