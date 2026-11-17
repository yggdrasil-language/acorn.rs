use core::fmt;

/// Why a DER element could not be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerErrorKind {
    /// The input ended in the middle of a header or value.
    Truncated,
    /// A length used the BER indefinite form or an absurdly wide long form.
    BadLength,
    /// A long-form length was not minimal for its value.
    NonMinimalLength,
    /// An INTEGER did not use the shortest two's-complement form.
    NonMinimalInteger,
    /// An INTEGER value did not fit the requested width.
    IntegerOverflow,
    /// The element tag did not match what the caller expected.
    TagMismatch,
    /// An OID or string content violated its type rules.
    MalformedContent,
    /// Bytes remained after the caller consumed the expected elements.
    TrailingData,
    /// A constructed value carried content that cannot be a valid child.
    InvalidChild,
}

/// The error returned by every reader and writer in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerError {
    /// What went wrong.
    pub kind: DerErrorKind,
    /// The tag the caller expected when the failure is a `TagMismatch`.
    pub expected: Option<u32>,
    /// The tag actually found when the failure is a `TagMismatch`.
    pub actual: Option<u32>,
    /// Byte offset where the failure was detected.
    pub offset: usize,
}

impl DerError {
    /// Build an error with its kind and offset.
    pub const fn new(kind: DerErrorKind, offset: usize) -> Self {
        Self { kind, expected: None, actual: None, offset }
    }

    /// Build a tag mismatch with the expected and actual tag numbers.
    pub const fn tag_mismatch(expected: u32, actual: u32, offset: usize) -> Self {
        Self { kind: DerErrorKind::TagMismatch, expected: Some(expected), actual: Some(actual), offset }
    }
}

impl fmt::Display for DerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DerErrorKind::Truncated => write!(formatter, "truncated DER element at offset {}", self.offset),
            DerErrorKind::BadLength => write!(formatter, "invalid DER length at offset {}", self.offset),
            DerErrorKind::NonMinimalLength => write!(formatter, "non-minimal DER length at offset {}", self.offset),
            DerErrorKind::NonMinimalInteger => write!(formatter, "non-minimal DER INTEGER at offset {}", self.offset),
            DerErrorKind::IntegerOverflow => write!(formatter, "DER INTEGER too large at offset {}", self.offset),
            DerErrorKind::TagMismatch => {
                write!(
                    formatter,
                    "DER tag mismatch at offset {}: expected {:?}, found {:?}",
                    self.offset, self.expected, self.actual
                )
            }
            DerErrorKind::MalformedContent => write!(formatter, "malformed DER content at offset {}", self.offset),
            DerErrorKind::TrailingData => write!(formatter, "trailing DER bytes at offset {}", self.offset),
            DerErrorKind::InvalidChild => write!(formatter, "invalid DER child at offset {}", self.offset),
        }
    }
}

impl std::error::Error for DerError {}

/// The result type shared by the whole crate.
pub type DerResult<T> = core::result::Result<T, DerError>;
