//! Tar-gzip container composition.

use std::fmt;

use acorn_codec::{
    CodecError, CodecLimits,
    compression::{Gzip, GzipStreamDecoder, StreamChunk, deflate::DeflateLevel},
};
use acorn_core::ByteRange;
use acorn_source::{ByteSource, ByteWindow, ReadOutcome, SourceRange};

use crate::{
    format::{BLOCK_SIZE, HeaderError, parse_header},
    model::{TarArchive, TarEntry, TarEntryKind},
    read::{MAX_METADATA_BYTES, PendingMetadata, SliceSource, TarIndexError, index_tar, padded_offset},
};

/// Default compressed read-ahead size of a tar-gzip stream.
pub const DEFAULT_STREAM_CHUNK: usize = 64 * 1024;

/// A decoded gzip stream together with its indexed tar payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarGzArchive {
    bytes: Vec<u8>,
    archive: TarArchive,
}

impl TarGzArchive {
    /// Indexed tar facts.
    pub fn archive(&self) -> &TarArchive {
        &self.archive
    }

    /// Decoded tar bytes.
    pub fn tar_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the payload bytes for an indexed member.
    pub fn payload(&self, entry: &TarEntry) -> Option<&[u8]> {
        let start = entry.data_range.start as usize;
        let end = start.checked_add(entry.data_range.len as usize)?;
        self.bytes.get(start..end)
    }
}

/// Tar-gzip composition failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarGzError {
    /// Gzip decoding or validation failed.
    Codec(CodecError),
    /// Bytes remain after the first gzip member.
    TrailingInput {
        /// Number of trailing bytes.
        bytes: usize,
    },
    /// The decoded bytes are not a valid tar archive.
    Tar(TarIndexError),
}

impl fmt::Display for TarGzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "gzip decode error: {error}"),
            Self::TrailingInput { bytes } => write!(f, "tar-gzip input has {bytes} trailing bytes after one member"),
            Self::Tar(error) => write!(f, "tar-gzip tar error: {error}"),
        }
    }
}

impl std::error::Error for TarGzError {}

impl From<CodecError> for TarGzError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<TarIndexError> for TarGzError {
    fn from(error: TarIndexError) -> Self {
        Self::Tar(error)
    }
}

/// Tar-gzip container operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct TarGz;

impl TarGz {
    /// Decodes one gzip member and indexes the tar payload inside it.
    pub fn decode(label: impl Into<String>, input: &[u8], limits: CodecLimits) -> Result<TarGzArchive, TarGzError> {
        let facts = Gzip::decode(input, limits)?;
        if facts.trailing_input != 0 {
            return Err(TarGzError::TrailingInput { bytes: facts.trailing_input });
        }
        let bytes = facts.value;
        let source = SliceSource::new(label, &bytes);
        let archive = index_tar(&source)?;
        Ok(TarGzArchive { bytes, archive })
    }

    /// Encodes tar bytes as a single-member gzip stream.
    pub fn encode(tar: &[u8]) -> Vec<u8> {
        Gzip::encode(tar, DeflateLevel::Default)
    }
}

/// Pulls decoded bytes from a gzip member over a random-access source.
struct GzipReader<S: ByteSource> {
    decoder: GzipStreamDecoder,
    window: ByteWindow<S>,
    position: u64,
    compressed_size: u64,
    chunk: Vec<u8>,
    decoded: u64,
    finished: bool,
}

impl<S: ByteSource> GzipReader<S> {
    fn new(source: S, limits: CodecLimits, chunk_size: usize) -> Result<Self, TarGzError> {
        let compressed_size = source.length().ok_or(TarGzError::Tar(TarIndexError::UnknownLength))?;
        let range =
            SourceRange::new(0, compressed_size).map_err(|error| TarGzError::Tar(TarIndexError::Source(error.to_string())))?;
        let window =
            ByteWindow::new(source, 0, range).map_err(|error| TarGzError::Tar(TarIndexError::Source(error.to_string())))?;
        Ok(Self {
            decoder: GzipStreamDecoder::new(limits),
            window,
            position: 0,
            compressed_size,
            chunk: vec![0u8; chunk_size.max(1)],
            decoded: 0,
            finished: false,
        })
    }

    fn decoded_bytes(&self) -> u64 {
        self.decoded
    }

    fn chunk_size(&self) -> usize {
        self.chunk.len()
    }

    fn trailing(&self) -> u64 {
        self.decoder.trailing_input() + self.compressed_size.saturating_sub(self.position)
    }

    /// Produces at most `out.len()` decoded bytes, pulling compressed input as needed.
    fn pull(&mut self, out: &mut [u8]) -> Result<StreamChunk, TarGzError> {
        if self.finished {
            return Ok(StreamChunk::Finished);
        }
        loop {
            match self.decoder.read(out).map_err(TarGzError::Codec)? {
                StreamChunk::Produced(count) => {
                    self.decoded += count as u64;
                    return Ok(StreamChunk::Produced(count));
                }
                StreamChunk::Finished => {
                    self.finished = true;
                    return Ok(StreamChunk::Finished);
                }
                StreamChunk::NeedInput => {
                    if self.position >= self.compressed_size {
                        return Err(TarGzError::Codec(CodecError::InputExhausted {
                            offset: self.position as usize,
                            needed: 1,
                        }));
                    }
                    let take = (self.chunk.len() as u64).min(self.compressed_size - self.position) as usize;
                    let bytes_read = match self.window.read_at(self.position, &mut self.chunk[..take]) {
                        Ok(ReadOutcome::Complete) => take,
                        Ok(ReadOutcome::Partial { bytes_read }) => bytes_read as usize,
                        Ok(ReadOutcome::Eof) => {
                            return Err(TarGzError::Codec(CodecError::InputExhausted {
                                offset: self.position as usize,
                                needed: 1,
                            }));
                        }
                        Ok(ReadOutcome::Unavailable { range, reason }) => {
                            let range = ByteRange::new_allowing_empty(range.start, range.len)
                                .unwrap_or(ByteRange { start: range.start, len: 0 });
                            return Err(TarGzError::Tar(TarIndexError::NeedRange { range, reason }));
                        }
                        Err(error) => return Err(TarGzError::Tar(TarIndexError::Source(error.to_string()))),
                    };
                    if bytes_read == 0 {
                        return Err(TarGzError::Codec(CodecError::InputExhausted {
                            offset: self.position as usize,
                            needed: 1,
                        }));
                    }
                    self.decoder.push(&self.chunk[..bytes_read]).map_err(TarGzError::Codec)?;
                    self.position += bytes_read as u64;
                }
            }
        }
    }

    fn read_exact(&mut self, length: u64) -> Result<Vec<u8>, TarGzError> {
        let mut bytes = Vec::with_capacity(length as usize);
        let mut buffer = vec![0u8; (length as usize).min(self.chunk_size())];
        while (bytes.len() as u64) < length {
            let want = ((length - bytes.len() as u64) as usize).min(buffer.len());
            match self.pull(&mut buffer[..want])? {
                StreamChunk::Produced(count) if count > 0 => bytes.extend_from_slice(&buffer[..count]),
                StreamChunk::Produced(_) | StreamChunk::Finished => {
                    return Err(TarGzError::Tar(TarIndexError::Truncated { offset: self.decoded }));
                }
                StreamChunk::NeedInput => unreachable!("the reader resolves input needs"),
            }
        }
        Ok(bytes)
    }

    fn skip(&mut self, length: u64) -> Result<(), TarGzError> {
        let mut remaining = length;
        let mut buffer = vec![0u8; (length as usize).min(self.chunk_size())];
        while remaining > 0 {
            let want = (remaining as usize).min(buffer.len());
            match self.pull(&mut buffer[..want])? {
                StreamChunk::Produced(count) if count > 0 => remaining -= count as u64,
                StreamChunk::Produced(_) | StreamChunk::Finished => {
                    return Err(TarGzError::Tar(TarIndexError::Truncated { offset: self.decoded }));
                }
                StreamChunk::NeedInput => unreachable!("the reader resolves input needs"),
            }
        }
        Ok(())
    }
}

/// Streams a tar-gzip archive member by member.
///
/// [`TarGzStream::next_entry`] returns one entry whose payload is read on
/// demand. The caller must consume or drop the entry reader before asking for
/// the next member, and the stream skips whatever was left. Only one gzip
/// decoder and one bounded compressed buffer stay live, so peak memory does not
/// follow the archive size.
pub struct TarGzStream<S: ByteSource> {
    reader: GzipReader<S>,
    pending: PendingMetadata,
    entry_end: u64,
    finished: bool,
}

impl<S: ByteSource> TarGzStream<S> {
    /// Opens a tar-gzip stream with [`DEFAULT_STREAM_CHUNK`] read-ahead.
    pub fn open(source: S, limits: CodecLimits) -> Result<Self, TarGzError> {
        Self::with_chunk_size(source, limits, DEFAULT_STREAM_CHUNK)
    }

    /// Opens a tar-gzip stream with an explicit compressed read-ahead size.
    pub fn with_chunk_size(source: S, limits: CodecLimits, chunk_size: usize) -> Result<Self, TarGzError> {
        Ok(Self {
            reader: GzipReader::new(source, limits, chunk_size)?,
            pending: PendingMetadata::default(),
            entry_end: 0,
            finished: false,
        })
    }

    /// Returns how many decoded tar bytes the stream produced.
    pub fn decoded_bytes(&self) -> u64 {
        self.reader.decoded_bytes()
    }

    /// Returns whether the end-of-archive block was seen.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Reads the next member, skipping the rest of the previous one.
    ///
    /// `Ok(None)` means the end-of-archive block was reached and the gzip
    /// trailer validated. Metadata records are applied to the following member
    /// exactly as in [`index_tar`].
    pub fn next_entry(&mut self) -> Result<Option<TarGzEntry<'_, S>>, TarGzError> {
        if self.finished {
            return Ok(None);
        }
        let target = self.entry_end;
        let decoded = self.reader.decoded_bytes();
        if target > decoded {
            self.reader.skip(target - decoded)?;
        }

        loop {
            let offset = self.reader.decoded_bytes();
            let block = self.reader.read_exact(BLOCK_SIZE)?;
            if block.iter().all(|&byte| byte == 0) {
                self.finished = true;
                self.drain_remaining()?;
                return Ok(None);
            }
            let header = parse_header(&block).map_err(|error| match error {
                HeaderError::ChecksumMismatch { expected, actual } => {
                    TarGzError::Tar(TarIndexError::ChecksumMismatch { offset, expected, actual })
                }
                HeaderError::InvalidField(field) => TarGzError::Tar(TarIndexError::InvalidHeaderField { offset, field }),
            })?;
            let data_offset = offset + BLOCK_SIZE;
            match header.typeflag {
                b'L' => {
                    self.pending.long_name = Some(self.read_metadata(header.size, offset)?);
                    continue;
                }
                b'K' => {
                    self.pending.long_link = Some(self.read_metadata(header.size, offset)?);
                    continue;
                }
                b'x' => {
                    let payload = self.read_metadata(header.size, offset)?;
                    self.pending.apply_pax(payload, data_offset)?;
                    continue;
                }
                b'g' => {
                    self.read_metadata(header.size, offset)?;
                    continue;
                }
                _ => {}
            }

            let (path, link_name, size, modified_unix) = self.pending.resolve(&header);
            let data_end =
                data_offset.checked_add(size).ok_or(TarGzError::Tar(TarIndexError::Truncated { offset: data_offset }))?;
            let data_range = ByteRange::new_allowing_empty(data_offset, size)
                .map_err(|_| TarGzError::Tar(TarIndexError::InvalidHeaderField { offset, field: "data range" }))?;
            self.entry_end = padded_offset(data_end);
            let entry = TarEntry {
                path,
                link_name,
                kind: TarEntryKind::from_typeflag(header.typeflag),
                mode: header.mode,
                uid: header.uid,
                gid: header.gid,
                size,
                modified_unix,
                header_offset: offset,
                data_range,
            };
            return Ok(Some(TarGzEntry { entry, reader: &mut self.reader, read: 0, remaining: size }));
        }
    }

    fn read_metadata(&mut self, size: u64, header_offset: u64) -> Result<Vec<u8>, TarGzError> {
        if size > MAX_METADATA_BYTES {
            return Err(TarGzError::Tar(TarIndexError::MetadataTooLarge { offset: header_offset, size }));
        }
        let mut bytes = self.reader.read_exact(size)?;
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        self.reader.skip(padded_offset(size) - size)?;
        Ok(bytes)
    }

    /// Consumes the rest of the gzip member so the trailer and input end are validated.
    fn drain_remaining(&mut self) -> Result<(), TarGzError> {
        let mut buffer = vec![0u8; self.reader.chunk_size()];
        loop {
            match self.reader.pull(&mut buffer)? {
                StreamChunk::Produced(_) => continue,
                StreamChunk::Finished => break,
                StreamChunk::NeedInput => unreachable!("the reader resolves input needs"),
            }
        }
        let trailing = self.reader.trailing();
        if trailing != 0 {
            return Err(TarGzError::TrailingInput { bytes: trailing as usize });
        }
        Ok(())
    }
}

/// One member yielded by [`TarGzStream::next_entry`].
pub struct TarGzEntry<'a, S: ByteSource> {
    entry: TarEntry,
    reader: &'a mut GzipReader<S>,
    read: u64,
    remaining: u64,
}

impl<S: ByteSource> TarGzEntry<'_, S> {
    /// Returns the indexed member metadata.
    pub fn entry(&self) -> &TarEntry {
        &self.entry
    }

    /// Returns how many payload bytes this reader delivered.
    pub fn read_bytes(&self) -> u64 {
        self.read
    }

    /// Returns how many payload bytes remain.
    pub fn remaining_bytes(&self) -> u64 {
        self.remaining
    }

    /// Produces at most `out.len()` payload bytes.
    ///
    /// The final bytes are returned as `Produced`, and the following call
    /// reports `Finished`. Dropping the reader before that makes the stream
    /// skip the rest.
    pub fn read(&mut self, out: &mut [u8]) -> Result<StreamChunk, TarGzError> {
        if self.remaining == 0 {
            return Ok(StreamChunk::Finished);
        }
        if out.is_empty() {
            return Ok(StreamChunk::Produced(0));
        }
        let take = (out.len() as u64).min(self.remaining) as usize;
        match self.reader.pull(&mut out[..take])? {
            StreamChunk::Produced(count) if count > 0 => {
                self.read += count as u64;
                self.remaining -= count as u64;
                Ok(StreamChunk::Produced(count))
            }
            StreamChunk::Produced(_) | StreamChunk::Finished => {
                Err(TarGzError::Tar(TarIndexError::Truncated { offset: self.reader.decoded_bytes() }))
            }
            StreamChunk::NeedInput => unreachable!("the reader resolves input needs"),
        }
    }
}
