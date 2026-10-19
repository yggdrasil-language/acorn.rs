use acorn_core::ByteRange;

/// Where a signature pattern is anchored inside a byte source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProbeAnchor {
    /// File start.
    Start,
    /// Fixed offset from the start of the current view.
    Fixed(
        /// Absolute offset from the start of the surrounding view.
        u64,
    ),
    /// Tail window of `len` bytes ending at EOF.
    Tail {
        /// Window length counted backwards from EOF.
        len: u64,
    },
    /// Bounded search window `[start, start + len)`.
    Search {
        /// Offset where the search window begins.
        start: u64,
        /// Maximum bytes searched inside the window.
        len: u64,
    },
}

/// Bytes with an optional per-byte mask (`0xFF` = must match).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BytePattern {
    /// Expected bytes of the pattern.
    pub bytes: Vec<u8>,
    /// Per-byte mask applied to both pattern and candidate.
    pub mask: Vec<u8>,
}

impl BytePattern {
    /// Exact byte sequence with full mask.
    pub fn exact(bytes: &[u8]) -> Self {
        Self { bytes: bytes.to_vec(), mask: vec![0xFF; bytes.len()] }
    }

    /// Returns whether `candidate` matches this pattern at offset zero.
    pub fn matches_at(&self, candidate: &[u8]) -> bool {
        if candidate.len() < self.bytes.len() {
            return false;
        }
        candidate
            .iter()
            .zip(self.bytes.iter().zip(self.mask.iter()))
            .all(|(actual, (expected, mask))| (*actual & *mask) == (*expected & *mask))
    }
}

/// Registered signature for one format adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Format identifier reported for a hit.
    pub format_id: String,
    /// Anchor deciding which byte range is read for evidence.
    pub anchor: ProbeAnchor,
    /// Masked byte pattern matched at the anchor.
    pub pattern: BytePattern,
    /// Ordering priority, larger values are listed first.
    pub priority: u16,
}

impl Signature {
    /// Creates a start-of-file signature.
    pub fn at_start(format_id: impl Into<String>, bytes: &[u8]) -> Self {
        Self { format_id: format_id.into(), anchor: ProbeAnchor::Start, pattern: BytePattern::exact(bytes), priority: 0 }
    }

    /// Resolves the byte range to read for this anchor given `source_len`.
    pub fn evidence_range(&self, source_len: u64) -> Option<ByteRange> {
        let pattern_len = self.pattern.bytes.len() as u64;
        if pattern_len == 0 || source_len == 0 {
            return None;
        }
        match self.anchor {
            ProbeAnchor::Start => {
                if source_len < pattern_len {
                    return None;
                }
                Some(ByteRange { start: 0, len: pattern_len })
            }
            ProbeAnchor::Fixed(offset) => {
                if offset + pattern_len > source_len {
                    return None;
                }
                Some(ByteRange { start: offset, len: pattern_len })
            }
            ProbeAnchor::Tail { len } => {
                let window = len.min(source_len);
                if window < pattern_len {
                    return None;
                }
                Some(ByteRange { start: source_len - window, len: window })
            }
            ProbeAnchor::Search { start, len } => {
                if start >= source_len {
                    return None;
                }
                let available = len.min(source_len - start);
                if available < pattern_len {
                    return None;
                }
                Some(ByteRange { start, len: available })
            }
        }
    }
}
