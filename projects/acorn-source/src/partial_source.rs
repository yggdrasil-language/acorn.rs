use crate::capabilities::SourceCapabilities;
use crate::identity::SourceIdentity;
use crate::read::{ReadOutcome, ReadResult, SourceRange};
use crate::source::ByteSource;

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
        Self {
            identity: SourceIdentity::new(label),
            segments,
            total_len: None,
        }
    }

    /// Declares the total length when known.
    pub fn with_total_length(mut self, total_len: u64) -> Self {
        self.total_len = Some(total_len);
        self
    }

    fn segment_for_offset(&self, offset: u64) -> Option<&[u8]> {
        self.segments
            .iter()
            .find(|(start, bytes)| offset >= *start && offset < start + bytes.len() as u64)
            .map(|(start, bytes)| {
                let local = (offset - start) as usize;
                &bytes[local..]
            })
    }
}

impl ByteSource for RangeMapSource {
    fn length(&self) -> Option<u64> {
        self.total_len
    }

    fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult {
        match self.segment_for_offset(offset) {
            Some(available) => {
                let to_copy = destination.len().min(available.len());
                destination[..to_copy].copy_from_slice(&available[..to_copy]);
                if to_copy == destination.len() {
                    Ok(ReadOutcome::Complete)
                } else {
                    Ok(ReadOutcome::Partial {
                        bytes_read: to_copy as u64,
                    })
                }
            }
            None => {
                let missing = SourceRange::new(offset, destination.len() as u64)
                    .unwrap_or(SourceRange { start: offset, len: 0 });
                Ok(ReadOutcome::Unavailable {
                    range: missing,
                    reason: "byte range not yet provided".to_string(),
                })
            }
        }
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::PARTIAL
    }

    fn identity(&self) -> SourceIdentity {
        self.identity.clone()
    }
}
