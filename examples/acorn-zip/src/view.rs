use acorn_core::ByteRange;
use acorn_source::{ByteSource, ByteWindow, ReadOutcome, SourceRange};

use crate::entry::ZipMember;
use crate::ZipIndexError;

/// ZIP compression method for stored (uncompressed) members.
pub const COMPRESSION_STORED: u16 = 0;

/// ZIP compression method for raw deflate (common in archives).
pub const COMPRESSION_DEFLATE: u16 = 8;

/// Errors opening a member payload view.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipViewError {
    /// Compression method is not supported for this operation.
    #[error("compression method {0} is not supported for payload views")]
    UnsupportedCompression(u16),
    /// Payload range is invalid for the archive.
    #[error("invalid member payload range")]
    InvalidRange,
    /// Underlying source window could not be created.
    #[error("window error: {0}")]
    Window(String),
    /// Deflate decompression failed.
    #[error("deflate decompression failed")]
    DecompressFailed,
    /// Declared and actual sizes do not match.
    #[error("member size mismatch")]
    SizeMismatch,
    /// Decode exceeded the parse budget.
    #[error("decode budget exceeded")]
    BudgetExceeded,
}

/// Returns the compressed payload byte range inside the archive file space.
pub fn member_payload_range(member: &ZipMember) -> ByteRange {
    member.payload_range
}

/// Reads the compressed or stored payload bytes from the archive without decoding.
pub fn read_raw_payload<S: ByteSource>(
    source: S,
    member: &ZipMember,
) -> Result<Vec<u8>, ZipViewError> {
    let range = member.payload_range;
    let source_range = SourceRange::new(range.start, range.len)
        .map_err(|error| ZipViewError::Window(error.to_string()))?;
    let window = ByteWindow::new(source, 0, source_range)
        .map_err(|error| ZipViewError::Window(error.to_string()))?;
    let len = window.length().ok_or(ZipViewError::InvalidRange)? as usize;
    let mut buffer = vec![0u8; len];
    match window.read_at(0, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(ReadOutcome::Eof) => Err(ZipViewError::InvalidRange),
        Ok(ReadOutcome::Unavailable { .. }) => Err(ZipViewError::InvalidRange),
        Err(error) => Err(ZipViewError::Window(error.to_string())),
    }
}

/// Opens a bounded random-access view over a stored member payload.
pub fn open_stored_member<S: ByteSource>(
    source: S,
    member: &ZipMember,
) -> Result<ByteWindow<S>, ZipViewError> {
    if member.compression_method != COMPRESSION_STORED {
        return Err(ZipViewError::UnsupportedCompression(
            member.compression_method,
        ));
    }
    let range = member.payload_range;
    let source_range = SourceRange::new(range.start, range.len)
        .map_err(|error| ZipViewError::Window(error.to_string()))?;
    ByteWindow::new(source, 0, source_range)
        .map_err(|error| ZipViewError::Window(error.to_string()))
}

/// Reads the full stored payload into a buffer.
pub fn read_stored_payload<S: ByteSource>(
    source: S,
    member: &ZipMember,
) -> Result<Vec<u8>, ZipViewError> {
    if member.compression_method != COMPRESSION_STORED {
        return Err(ZipViewError::UnsupportedCompression(
            member.compression_method,
        ));
    }
    read_raw_payload(source, member)
}

impl From<ZipViewError> for ZipIndexError {
    fn from(error: ZipViewError) -> Self {
        ZipIndexError::Layout(error.to_string())
    }
}
