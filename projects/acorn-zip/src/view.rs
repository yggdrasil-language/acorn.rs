use acorn_codec::{
    CodecError, CodecLimits,
    compression::{Crc32, DeflateStreamDecoder, StreamChunk},
};
use acorn_core::{AddressSpaceId, ByteRange, NeedRange};
use acorn_source::{ByteSource, ByteWindow, ReadOutcome, SourceRange};

use crate::{ZipIndexError, entry::ZipMember};

/// Default compressed read-ahead size of a streaming member.
pub const DEFAULT_STREAM_CHUNK: usize = 64 * 1024;

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

/// Streams one member payload in caller-sized chunks.
///
/// `read` decodes up to the destination length and returns the bytes produced,
/// `StreamChunk::NeedInput` never escapes to the caller, and
/// `StreamChunk::Finished` means the declared size and CRC matched. The caller
/// decides the output chunk size, while the reader keeps at most
/// [`DEFAULT_STREAM_CHUNK`] compressed bytes (or the configured limit) live.
///
/// Dropping the reader cancels the operation. It owns no background work, so
/// a partially read member leaves no state behind.
pub struct ZipMemberStream<S: ByteSource> {
    member: ZipMember,
    window: ByteWindow<S>,
    chunk: Vec<u8>,
    position: u64,
    decoded: u64,
    crc: Crc32,
    inner: StreamInner,
    finished: bool,
}

/// Decoder state that does not depend on the caller's buffer.
enum StreamInner {
    Stored,
    Deflate(DeflateStreamDecoder),
}

impl<S: ByteSource> ZipMemberStream<S> {
    /// Opens a streaming reader with [`DEFAULT_STREAM_CHUNK`] read-ahead.
    pub fn open(source: S, member: &ZipMember, limits: CodecLimits) -> Result<Self, ZipViewError> {
        Self::with_chunk_size(source, member, limits, DEFAULT_STREAM_CHUNK)
    }

    /// Opens a streaming reader with an explicit compressed read-ahead size.
    ///
    /// `chunk_size` bounds the compressed bytes held by the reader. It does not
    /// bound decoded output, which follows the destination passed to `read`.
    pub fn with_chunk_size(
        source: S,
        member: &ZipMember,
        limits: CodecLimits,
        chunk_size: usize,
    ) -> Result<Self, ZipViewError> {
        if member.payload_range.len != member.compressed_size {
            return Err(ZipViewError::SizeMismatch);
        }
        let inner = match member.compression_method {
            COMPRESSION_STORED => {
                if member.uncompressed_size != member.compressed_size {
                    return Err(ZipViewError::SizeMismatch);
                }
                StreamInner::Stored
            }
            COMPRESSION_DEFLATE => StreamInner::Deflate(DeflateStreamDecoder::new(limits)),
            other => return Err(ZipViewError::UnsupportedCompression(other)),
        };
        let range = member.payload_range;
        let source_range = SourceRange::new(range.start, range.len).map_err(|error| ZipViewError::Window(error.to_string()))?;
        let window = ByteWindow::new(source, 0, source_range).map_err(|error| ZipViewError::Window(error.to_string()))?;
        Ok(Self {
            member: member.clone(),
            window,
            chunk: vec![0u8; chunk_size.max(1)],
            position: 0,
            decoded: 0,
            crc: Crc32::new(),
            inner,
            finished: false,
        })
    }

    /// Returns the indexed member being streamed.
    pub fn member(&self) -> &ZipMember {
        &self.member
    }

    /// Returns how many uncompressed bytes this reader produced.
    pub fn decoded_bytes(&self) -> u64 {
        self.decoded
    }

    /// Returns how many compressed bytes the reader consumed.
    pub fn compressed_bytes(&self) -> u64 {
        self.position
    }

    /// Returns whether the declared size and CRC have been confirmed.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Decodes up to `out.len()` bytes.
    ///
    /// An empty destination yields `Produced(0)` so callers can poll without
    /// blocking. After `Finished` the call is idempotent.
    pub fn read(&mut self, out: &mut [u8]) -> Result<StreamChunk, ZipViewError> {
        if self.finished {
            return Ok(StreamChunk::Finished);
        }
        if out.is_empty() {
            return Ok(StreamChunk::Produced(0));
        }
        match self.member.compression_method {
            COMPRESSION_STORED => self.read_stored(out),
            _ => self.read_deflated(out),
        }
    }

    fn read_stored(&mut self, out: &mut [u8]) -> Result<StreamChunk, ZipViewError> {
        let remaining = self.member.uncompressed_size.saturating_sub(self.decoded);
        if remaining == 0 {
            return self.settle();
        }
        let take = (out.len() as u64).min(remaining) as usize;
        let bytes_read = match self.window.read_at(self.position, &mut out[..take]) {
            Ok(ReadOutcome::Complete) => take as u64,
            Ok(ReadOutcome::Partial { bytes_read }) => bytes_read,
            Ok(ReadOutcome::Eof) => return Err(ZipViewError::SizeMismatch),
            Ok(ReadOutcome::Unavailable { .. }) => return Err(ZipViewError::InvalidRange),
            Err(error) => return Err(ZipViewError::Window(error.to_string())),
        };
        let bytes_read = bytes_read as usize;
        self.position += bytes_read as u64;
        self.decoded += bytes_read as u64;
        self.crc.update(&out[..bytes_read]);
        if self.decoded == self.member.uncompressed_size {
            self.settle()?;
        }
        Ok(StreamChunk::Produced(bytes_read))
    }

    fn read_deflated(&mut self, out: &mut [u8]) -> Result<StreamChunk, ZipViewError> {
        let StreamInner::Deflate(decoder) = &mut self.inner
        else {
            unreachable!("deflated member")
        };
        let window = &mut self.window;
        let chunk = &mut self.chunk;
        let position = &mut self.position;
        let compressed_size = self.member.compressed_size;

        loop {
            match decoder.read(out) {
                Ok(StreamChunk::Produced(count)) => {
                    self.decoded += count as u64;
                    self.crc.update(&out[..count]);
                    return Ok(StreamChunk::Produced(count));
                }
                Ok(StreamChunk::Finished) => return self.settle(),
                Ok(StreamChunk::NeedInput) => {
                    if *position >= compressed_size {
                        return Err(ZipViewError::Codec(CodecError::InputExhausted { offset: *position as usize, needed: 1 }));
                    }
                    let take = (chunk.len() as u64).min(compressed_size - *position) as usize;
                    let bytes_read = match window.read_at(*position, &mut chunk[..take]) {
                        Ok(ReadOutcome::Complete) => take,
                        Ok(ReadOutcome::Partial { bytes_read }) => bytes_read as usize,
                        Ok(ReadOutcome::Eof) => return Err(ZipViewError::SizeMismatch),
                        Ok(ReadOutcome::Unavailable { .. }) => return Err(ZipViewError::InvalidRange),
                        Err(error) => return Err(ZipViewError::Window(error.to_string())),
                    };
                    if bytes_read == 0 {
                        return Err(ZipViewError::SizeMismatch);
                    }
                    decoder.push(&chunk[..bytes_read]).map_err(map_codec_failure)?;
                    *position += bytes_read as u64;
                }
                Err(error) => return Err(map_codec_failure(error)),
            }
        }
    }

    fn settle(&mut self) -> Result<StreamChunk, ZipViewError> {
        if self.decoded != self.member.uncompressed_size {
            return Err(ZipViewError::SizeMismatch);
        }
        let actual = self.crc.value();
        if actual != self.member.crc32 {
            return Err(ZipViewError::CrcMismatch { expected: self.member.crc32, actual });
        }
        self.finished = true;
        Ok(StreamChunk::Finished)
    }
}

/// Maps a codec failure while keeping resource limits distinguishable.
fn map_codec_failure(error: CodecError) -> ZipViewError {
    if error.is_resource_limit() { ZipViewError::BudgetExceeded } else { ZipViewError::Codec(error) }
}

impl From<ZipViewError> for ZipIndexError {
    fn from(error: ZipViewError) -> Self {
        ZipIndexError::Layout(error.to_string())
    }
}
