use acorn_core::ByteRange;
use acorn_source::{ByteSource, ByteWindow, ReadOutcome, SourceRange};

use crate::entry::ZipMember;
use crate::ZipIndexError;

/// ZIP compression method for stored (uncompressed) members.
pub const COMPRESSION_STORED: u16 = 0;

/// Errors opening a member payload view.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipViewError {
    /// Only stored members expose a direct payload window today.
    #[error("compression method {0} is not supported for payload views")]
    UnsupportedCompression(u16),
    /// Payload range is invalid for the archive.
    #[error("invalid member payload range")]
    InvalidRange,
    /// Underlying source window could not be created.
    #[error("window error: {0}")]
    Window(String),
}

/// Returns the compressed payload byte range inside the archive file space.
pub fn member_payload_range(member: &ZipMember) -> ByteRange {
    member.payload_range
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
    let window = open_stored_member(source, member)?;
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

impl From<ZipViewError> for ZipIndexError {
    fn from(error: ZipViewError) -> Self {
        ZipIndexError::Layout(error.to_string())
    }
}
