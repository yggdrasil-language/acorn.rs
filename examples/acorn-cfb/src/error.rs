use std::fmt;

/// Errors while reading compound files.
#[derive(Debug)]
pub enum CfbError {
    /// Input is shorter than required.
    TooSmall {
        /// Minimum bytes required.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Missing compound file signature.
    InvalidSignature,
    /// Unsupported major sector size exponent.
    InvalidSectorShift {
        /// Header sector shift value.
        sector_shift: u16,
    },
    /// Unsupported mini sector size exponent.
    InvalidMiniSectorShift {
        /// Header mini sector shift value.
        mini_sector_shift: u16,
    },
    /// Sector id is out of range for this file.
    SectorOutOfRange {
        /// Referenced sector id.
        sector_id: u32,
        /// Maximum valid sector index.
        max_sector: u32,
    },
    /// FAT chain ended unexpectedly.
    UnexpectedEndOfChain {
        /// Human-readable read context.
        context: String,
    },
    /// FAT chain exceeded the safety bound.
    ChainTooLong {
        /// Human-readable read context.
        context: String,
    },
    /// Directory entry name is invalid UTF-16.
    InvalidEntryName,
    /// Stream path was not found.
    StreamNotFound {
        /// Requested absolute stream path.
        path: String,
    },
    /// Path syntax is invalid.
    InvalidPath {
        /// Provided path string.
        path: String,
    },
    /// I/O error while loading compound file bytes.
    Io(std::io::Error),
    /// Stream size exceeds configured read bound.
    StreamTooLarge {
        /// Stream path.
        path: String,
        /// Declared stream size.
        size: u64,
        /// Configured limit.
        limit: u64,
    },
}

impl fmt::Display for CfbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall { expected, actual } => {
                write!(f, "input too small: expected at least {expected} bytes, got {actual}")
            }
            Self::InvalidSignature => write!(f, "invalid compound file signature"),
            Self::InvalidSectorShift { sector_shift } => {
                write!(f, "unsupported sector shift {sector_shift}")
            }
            Self::InvalidMiniSectorShift { mini_sector_shift } => {
                write!(f, "unsupported mini sector shift {mini_sector_shift}")
            }
            Self::SectorOutOfRange { sector_id, max_sector } => {
                write!(f, "sector id {sector_id} out of range (max {max_sector})")
            }
            Self::UnexpectedEndOfChain { context } => {
                write!(f, "unexpected end of FAT chain while reading {context}")
            }
            Self::ChainTooLong { context } => {
                write!(f, "FAT chain exceeded limit while reading {context}")
            }
            Self::InvalidEntryName => write!(f, "invalid directory entry name"),
            Self::StreamNotFound { path } => write!(f, "stream not found: {path}"),
            Self::InvalidPath { path } => write!(f, "invalid stream path: {path}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::StreamTooLarge { path, size, limit } => {
                write!(f, "stream {path} size {size} exceeds limit {limit}")
            }
        }
    }
}

impl std::error::Error for CfbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CfbError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Result alias for CFB operations.
pub type Result<T> = std::result::Result<T, CfbError>;
