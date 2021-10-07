/// Stable identity for cache keys, snapshots, and layout reuse.
///
/// A label and a version token describe where content came from, but they do not
/// prove that the bytes are unchanged. Only a matching content digest lets a
/// cached snapshot be reused, so [`SourceIdentity::snapshot_reuses`] refuses a
/// comparison where either side lacks a digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceIdentity {
    /// Opaque label, such as a file path, URI, or synthetic id.
    pub label: String,
    /// Optional content version token, such as an ETag or revision.
    pub version: Option<String>,
    /// Digest of the bytes this identity was built from, when they were materialized.
    pub content_digest: Option<u64>,
}

impl SourceIdentity {
    /// Creates a new identity.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), version: None, content_digest: None }
    }

    /// Attaches a version token.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Attaches the digest of the bytes this identity was built from.
    pub fn with_content_digest(mut self, digest: u64) -> Self {
        self.content_digest = Some(digest);
        self
    }

    /// Whether a snapshot built for `self` may be reused for `current`.
    ///
    /// The label and version token must match, and both sides must carry the same
    /// content digest. A missing digest on either side means the snapshot cannot be
    /// proven current, so reuse is refused instead of assumed.
    pub fn snapshot_reuses(&self, current: &SourceIdentity) -> bool {
        self.label == current.label
            && self.version == current.version
            && match (self.content_digest, current.content_digest) {
                (Some(built), Some(now)) => built == now,
                _ => false,
            }
    }
}

/// Digest of `bytes` for source snapshot identity.
///
/// This is an FNV-1a fold used to detect changed content. It is not a
/// cryptographic hash and must not be treated as a security boundary.
pub fn content_digest_of(bytes: &[u8]) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }
    digest
}
