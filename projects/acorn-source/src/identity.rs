/// Stable identity for cache keys and layout reuse.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceIdentity {
    /// Opaque label, such as a file path, URI, or synthetic id.
    pub label: String,
    /// Optional content version token, such as an ETag or revision.
    pub version: Option<String>,
}

impl SourceIdentity {
    /// Creates a new identity.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            version: None,
        }
    }

    /// Attaches a version token.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}
