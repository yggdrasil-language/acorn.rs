use crate::capabilities::SourceCapabilities;
use crate::identity::SourceIdentity;
use crate::read::{ReadOutcome, ReadResult};
use crate::source::ByteSource;

/// In-memory `ByteSource` backed by a byte slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySource {
    bytes: Vec<u8>,
    identity: SourceIdentity,
}

impl MemorySource {
    /// Creates a source from owned bytes.
    pub fn from_bytes(label: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            identity: SourceIdentity::new(label),
        }
    }

    /// Creates a source from a static slice.
    pub fn from_static(label: impl Into<String>, bytes: &'static [u8]) -> Self {
        Self::from_bytes(label, bytes.to_vec())
    }
}

impl ByteSource for MemorySource {
    fn length(&self) -> Option<u64> {
        Some(self.bytes.len() as u64)
    }

    fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult {
        let offset = offset as usize;
        if offset >= self.bytes.len() {
            return Ok(ReadOutcome::Eof);
        }
        let available = self.bytes.len() - offset;
        let to_copy = destination.len().min(available);
        destination[..to_copy].copy_from_slice(&self.bytes[offset..offset + to_copy]);
        if to_copy == destination.len() {
            Ok(ReadOutcome::Complete)
        } else {
            Ok(ReadOutcome::Partial {
                bytes_read: to_copy as u64,
            })
        }
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::MEMORY
    }

    fn identity(&self) -> SourceIdentity {
        self.identity.clone()
    }
}
