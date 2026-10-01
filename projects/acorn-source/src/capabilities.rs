/// Describes what a `ByteSource` implementation can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceCapabilities {
    /// Supports random access reads.
    pub random_access: bool,
    /// Reports a stable total length.
    pub known_length: bool,
    /// Repeated reads return identical bytes.
    pub repeatable: bool,
    /// Safe to read concurrently from multiple threads.
    pub concurrent_reads: bool,
    /// Exposes a stable `SourceIdentity`.
    pub stable_identity: bool,
    /// Allows creating decoded sub-views.
    pub decoded_views: bool,
}

impl SourceCapabilities {
    /// Capabilities of an in-memory slice.
    pub const MEMORY: Self = Self {
        random_access: true,
        known_length: true,
        repeatable: true,
        concurrent_reads: true,
        stable_identity: true,
        decoded_views: true,
    };

    /// Capabilities of a partial or remote range source.
    pub const PARTIAL: Self = Self {
        random_access: true,
        known_length: false,
        repeatable: true,
        concurrent_reads: false,
        stable_identity: true,
        decoded_views: false,
    };
}
