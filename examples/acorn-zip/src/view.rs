use acorn_codec::CodecError;
use acorn_core::{AddressSpaceId, ByteRange, NeedRange};
use acorn_source::{ByteSource, ByteWindow, ReadOutcome, SourceRange};

use crate::{ZipIndexError, entry::ZipMember};

/// ZIP compression method for stored (uncompressed) members.
pub const COMPRESSION_STORED: u16 = 0;

/// ZIP compression method for raw deflate (common in archives).
pub const COMPRESSION_DEFLATE: u16 = 8;

/// Errors opening a member payload view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipViewError {
    /// Compression method is not supported for this operation.
    UnsupportedCompression(u16),
    /// Payload range is invalid for the archive.
    InvalidRange,
    /// Underlying source window could not be created.
    Window(String),
    /// Codec failure with its own category, byte offsets, and bit positions preserved.
    Codec(CodecError),
    /// Declared and actual sizes do not match.
    SizeMismatch,
    /// Decode exceeded the parse budget.
    BudgetExceeded,
    /// The source does not currently hold a required interval.
    ///
    /// The caller may supply the interval and read again. Reporting this instead of an
    /// invalid member keeps a not-yet-fetched archive distinct from a broken one.
    NeedRange {
        /// Missing interval in the archive file space.
        range: ByteRange,
    },
    /// Decoded CRC32 does not match the central-directory CRC32.
    CrcMismatch {
        /// CRC32 declared by the central directory.
        expected: u32,
        /// CRC32 of the decoded payload.
        actual: u32,
    },
    /// Compressed input was not fully consumed by the decoder.
    IncompleteCompressedInput,
}

impl ZipViewError {
    /// Builds the core range request for this failure, when it is one.
    ///
    /// A caller that owns a provider can ask for exactly the interval the member could
    /// not supply, instead of reporting the member as invalid.
    pub fn need_range(&self, space: AddressSpaceId, reason: &str) -> Option<NeedRange> {
        match self {
            Self::NeedRange { range } => Some(NeedRange { space, range: *range, reason: reason.to_string() }),
            _ => None,
        }
    }
}

impl std::fmt::Display for ZipViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedCompression(method) => {
                write!(f, "compression method {method} is not supported for payload views")
            }
            Self::InvalidRange => write!(f, "invalid member payload range"),
            Self::Window(message) => write!(f, "window error: {message}"),
            Self::Codec(error) => write!(f, "member codec error: {error}"),
            Self::SizeMismatch => write!(f, "member size mismatch"),
            Self::BudgetExceeded => write!(f, "decode budget exceeded"),
            Self::NeedRange { range } => {
                write!(f, "source does not hold bytes for range {}..{}", range.start, range.start + range.len)
            }
            Self::CrcMismatch { .. } => write!(f, "member CRC32 mismatch"),
            Self::IncompleteCompressedInput => write!(f, "compressed input was not fully consumed"),
        }
    }
}

impl std::error::Error for ZipViewError {}

/// Returns the compressed payload byte range inside the archive file space.
pub fn member_payload_range(member: &ZipMember) -> ByteRange {
    member.payload_range
}

/// Reads the compressed or stored payload bytes from the archive without decoding.
pub fn read_raw_payload<S: ByteSource>(source: S, member: &ZipMember) -> Result<Vec<u8>, ZipViewError> {
    let range = member.payload_range;
    if range.len == 0 {
        return Ok(Vec::new());
    }
    let source_range = SourceRange::new(range.start, range.len).map_err(|error| ZipViewError::Window(error.to_string()))?;
    let window = ByteWindow::new(source, 0, source_range).map_err(|error| ZipViewError::Window(error.to_string()))?;
    let len = window.length().ok_or(ZipViewError::InvalidRange)? as usize;
    let mut buffer = vec![0u8; len];
    match window.read_at(0, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(ReadOutcome::Eof) => Err(ZipViewError::InvalidRange),
        Ok(ReadOutcome::Unavailable { range, .. }) => {
            let range = ByteRange::new_allowing_empty(range.start, range.len)
                .map_err(|error| ZipViewError::Window(error.to_string()))?;
            Err(ZipViewError::NeedRange { range })
        }
        Err(error) => Err(ZipViewError::Window(error.to_string())),
    }
}

/// Opens a bounded random-access view over a stored member payload.
pub fn open_stored_member<S: ByteSource>(source: S, member: &ZipMember) -> Result<ByteWindow<S>, ZipViewError> {
    if member.compression_method != COMPRESSION_STORED {
        return Err(ZipViewError::UnsupportedCompression(member.compression_method));
    }
    let range = member.payload_range;
    let source_range = SourceRange::new(range.start, range.len).map_err(|error| ZipViewError::Window(error.to_string()))?;
    ByteWindow::new(source, 0, source_range).map_err(|error| ZipViewError::Window(error.to_string()))
}

/// Reads the full stored payload into a buffer.
pub fn read_stored_payload<S: ByteSource>(source: S, member: &ZipMember) -> Result<Vec<u8>, ZipViewError> {
    if member.compression_method != COMPRESSION_STORED {
        return Err(ZipViewError::UnsupportedCompression(member.compression_method));
    }
    read_raw_payload(source, member)
}

impl From<ZipViewError> for ZipIndexError {
    fn from(error: ZipViewError) -> Self {
        ZipIndexError::Layout(error.to_string())
    }
}
