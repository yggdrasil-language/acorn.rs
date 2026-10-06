use thiserror::Error;

/// Errors while reading compound files.
#[derive(Debug, Error)]
pub enum CfbError {
    /// Input is shorter than required.
    #[error("input too small: expected at least {expected} bytes, got {actual}")]
    TooSmall {
        /// Minimum bytes required.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Missing compound file signature.
    #[error("invalid compound file signature")]
    InvalidSignature,
    /// Unsupported major sector size exponent.
    #[error("unsupported sector shift {sector_shift}")]
    InvalidSectorShift {
        /// Header sector shift value.
        sector_shift: u16,
    },
    /// Unsupported mini sector size exponent.
    #[error("unsupported mini sector shift {mini_sector_shift}")]
    InvalidMiniSectorShift {
        /// Header mini sector shift value.
        mini_sector_shift: u16,
    },
    /// Sector id is out of range for this file.
    #[error("sector id {sector_id} out of range (max {max_sector})")]
    SectorOutOfRange {
        /// Referenced sector id.
        sector_id: u32,
        /// Maximum valid sector index.
        max_sector: u32,
    },
    /// FAT chain ended unexpectedly.
    #[error("unexpected end of FAT chain while reading {context}")]
    UnexpectedEndOfChain {
        /// Human-readable read context.
        context: String,
    },
    /// FAT chain exceeded the safety bound.
    #[error("FAT chain exceeded limit while reading {context}")]
    ChainTooLong {
        /// Human-readable read context.
        context: String,
    },
    /// Directory entry name is invalid UTF-16.
    #[error("invalid directory entry name")]
    InvalidEntryName,
    /// Stream path was not found.
    #[error("stream not found: {path}")]
    StreamNotFound {
        /// Requested absolute stream path.
        path: String,
    },
    /// Path syntax is invalid.
    #[error("invalid stream path: {path}")]
    InvalidPath {
        /// Provided path string.
        path: String,
    },
    /// I/O error while loading compound file bytes.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Stream size exceeds configured read bound.
    #[error("stream {path} size {size} exceeds limit {limit}")]
    StreamTooLarge {
        /// Stream path.
        path: String,
        /// Declared stream size.
        size: u64,
        /// Configured limit.
        limit: u64,
    },
}

/// Result alias for CFB operations.
pub type Result<T> = std::result::Result<T, CfbError>;
