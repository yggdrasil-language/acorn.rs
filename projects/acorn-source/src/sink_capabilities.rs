/// Describes what a `ByteSink` implementation can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SinkCapabilities {
    /// Supports appending bytes at the current end.
    pub sequential_append: bool,
    /// Supports writing at arbitrary offsets.
    pub seek_write: bool,
    /// Supports patching previously written bytes in place.
    pub patch_in_place: bool,
    /// Reports the current written length.
    pub known_length: bool,
}

impl SinkCapabilities {
    /// Capabilities of an in-memory growable sink.
    pub const MEMORY: Self = Self {
        sequential_append: true,
        seek_write: true,
        patch_in_place: true,
        known_length: true,
    };

    /// Capabilities of a strictly sequential sink.
    pub const SEQUENTIAL: Self = Self {
        sequential_append: true,
        seek_write: false,
        patch_in_place: false,
        known_length: true,
    };
}
