//! Tar header scanning and payload reads.

use std::fmt;

use acorn_codec::compression::StreamChunk;
use acorn_core::{BudgetError, BudgetTracker, ByteRange};
use acorn_source::{
    ByteSource, ByteWindow, MemorySource, ReadError, ReadOutcome, ReadResult, SourceCapabilities, SourceIdentity, SourceRange,
    content_digest_of,
};

use crate::{
    format::{BLOCK_SIZE, Header, HeaderError, parse_header},
    model::{TarArchive, TarEntry, TarEntryKind},
};

/// Largest metadata payload accepted for GNU long names and PAX records.
pub(crate) const MAX_METADATA_BYTES: u64 = 1024 * 1024;

/// Tar indexing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarIndexError {
    /// The source does not report a length.
    UnknownLength,
    /// A header or payload extends past the known source length.
    Truncated {
        /// Offset where the expected data starts.
        offset: u64,
    },
    /// The archive ends without an end-of-archive zero block.
    MissingEndOfArchive {
        /// Offset where a header or end block was expected.
        offset: u64,
    },
    /// Header checksum does not match its bytes.
    ChecksumMismatch {
        /// Header offset.
        offset: u64,
        /// Checksum stored in the header.
        expected: u32,
        /// Checksum computed from the header bytes.
        actual: u32,
    },
    /// A numeric or fixed header field cannot be interpreted.
    InvalidHeaderField {
        /// Header offset.
        offset: u64,
        /// Field name.
        field: &'static str,
    },
    /// A GNU or PAX metadata record is too large to load.
    MetadataTooLarge {
        /// Header offset.
        offset: u64,
        /// Declared metadata size.
        size: u64,
    },
    /// A metadata record is not followed by a member.
    MetadataWithoutEntry {
        /// Offset after the metadata payload.
        offset: u64,
    },
    /// A PAX record is malformed.
    InvalidPaxRecord {
        /// Offset of the PAX payload.
        offset: u64,
    },
    /// Bytes needed for indexing are not available.
    NeedRange {
        /// Missing interval.
        range: ByteRange,
        /// Why the interval is unavailable.
        reason: String,
    },
    /// A source-level failure other than a missing range.
    Source(String),
}

impl fmt::Display for TarIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLength => write!(f, "tar source length is unknown"),
            Self::Truncated { offset } => write!(f, "tar is truncated at offset {offset}"),
            Self::MissingEndOfArchive { offset } => write!(f, "tar ends without a zero block at offset {offset}"),
            Self::ChecksumMismatch { offset, expected, actual } => {
                write!(f, "tar checksum mismatch at offset {offset}: expected {expected}, got {actual}")
            }
            Self::InvalidHeaderField { offset, field } => {
                write!(f, "tar header field {field} is invalid at offset {offset}")
            }
            Self::MetadataTooLarge { offset, size } => {
                write!(f, "tar metadata at offset {offset} is too large: {size} bytes")
            }
            Self::MetadataWithoutEntry { offset } => {
                write!(f, "tar metadata at offset {offset} is not followed by a member")
            }
            Self::InvalidPaxRecord { offset } => write!(f, "tar PAX record at offset {offset} is malformed"),
            Self::NeedRange { range, reason } => {
                write!(f, "tar range {}..{} is unavailable: {reason}", range.start, range.start + range.len)
            }
            Self::Source(message) => write!(f, "tar source error: {message}"),
        }
    }
}

impl std::error::Error for TarIndexError {}

/// Payload read failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarReadError {
    /// A budget reservation or settlement failed.
    Budget(BudgetError),
    /// Bytes needed for the payload are not available.
    NeedRange {
        /// Missing interval.
        range: ByteRange,
        /// Why the interval is unavailable.
        reason: String,
    },
    /// A range cannot be represented.
    InvalidRange(String),
    /// A source-level failure.
    Source(String),
}

impl fmt::Display for TarReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(error) => write!(f, "tar payload budget error: {error}"),
            Self::NeedRange { range, reason } => {
                write!(f, "tar payload range {}..{} is unavailable: {reason}", range.start, range.start + range.len)
            }
            Self::InvalidRange(message) => write!(f, "tar payload range is invalid: {message}"),
            Self::Source(message) => write!(f, "tar payload source error: {message}"),
        }
    }
}

impl std::error::Error for TarReadError {}

impl From<BudgetError> for TarReadError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

/// Indexes tar headers without materializing member payloads.
pub fn index_tar<S: ByteSource>(source: &S) -> Result<TarArchive, TarIndexError> {
    let length = source.length().ok_or(TarIndexError::UnknownLength)?;
    let mut entries = Vec::new();
    let mut offset = 0u64;
    let mut pending = PendingMetadata::default();

    loop {
        if offset >= length {
            if length == 0 {
                return Ok(TarArchive { identity: source.identity(), entries, end_of_archive: 0 });
            }
            return Err(TarIndexError::MissingEndOfArchive { offset });
        }
        let block = read_block(source, offset, length)?;
        if block.iter().all(|&byte| byte == 0) {
            break;
        }

        let header = parse_header(&block).map_err(|error| match error {
            HeaderError::ChecksumMismatch { expected, actual } => TarIndexError::ChecksumMismatch { offset, expected, actual },
            HeaderError::InvalidField(field) => TarIndexError::InvalidHeaderField { offset, field },
        })?;
        let data_offset = offset + BLOCK_SIZE;
        let data_end = data_offset.checked_add(header.size).ok_or(TarIndexError::Truncated { offset: data_offset })?;
        if data_end > length {
            return Err(TarIndexError::Truncated { offset: data_offset });
        }

        match header.typeflag {
            b'L' => {
                pending.long_name = Some(read_metadata(source, data_offset, header.size, offset)?);
                offset = padded_offset(data_end);
                continue;
            }
            b'K' => {
                pending.long_link = Some(read_metadata(source, data_offset, header.size, offset)?);
                offset = padded_offset(data_end);
                continue;
            }
            b'x' => {
                pending.apply_pax(read_metadata(source, data_offset, header.size, offset)?, data_offset)?;
                offset = padded_offset(data_end);
                continue;
            }
            b'g' => {
                offset = padded_offset(data_end);
                continue;
            }
            _ => {}
        }

        let (path, link_name, size, modified_unix) = pending.resolve(&header);
        let data_end = data_offset.checked_add(size).ok_or(TarIndexError::Truncated { offset: data_offset })?;
        if data_end > length {
            return Err(TarIndexError::Truncated { offset: data_offset });
        }
        let data_range = ByteRange::new_allowing_empty(data_offset, size)
            .map_err(|error| TarIndexError::InvalidHeaderField { offset, field: error_field(error) })?;
        entries.push(TarEntry {
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
        });
        offset = padded_offset(data_end);
    }

    if pending.has_values() {
        return Err(TarIndexError::MetadataWithoutEntry { offset });
    }

    Ok(TarArchive { identity: source.identity(), entries, end_of_archive: offset })
}

/// Indexes tar bytes held in memory.
pub fn index_tar_bytes(label: impl Into<String>, bytes: Vec<u8>) -> Result<TarArchive, TarIndexError> {
    index_tar(&MemorySource::from_bytes(label, bytes))
}

/// Reads one member payload into memory after reserving the read budget.
pub fn read_member_payload<S: ByteSource>(
    source: S,
    entry: &TarEntry,
    ledger: &mut BudgetTracker,
) -> Result<Vec<u8>, TarReadError> {
    let range = entry.data_range;
    if range.len == 0 {
        return Ok(Vec::new());
    }
    ledger.reserve_read_bytes(range.len)?;
    let source_range = match SourceRange::new(range.start, range.len) {
        Ok(range) => range,
        Err(error) => {
            ledger.release_read_bytes(entry.data_range.len)?;
            return Err(TarReadError::InvalidRange(error.to_string()));
        }
    };
    match source.read_range(source_range) {
        Ok(bytes) => {
            ledger.settle_read_bytes(bytes.len() as u64)?;
            Ok(bytes)
        }
        Err(error) => {
            ledger.release_read_bytes(entry.data_range.len)?;
            Err(map_read_error(error))
        }
    }
}

/// Streams one indexed member payload in caller-sized chunks.
///
/// The reader copies directly from the source range into the destination, so
/// peak memory follows the destination length and never the member size.
/// Dropping the reader cancels the member without leaving shared state.
pub struct TarEntryStream<S: ByteSource> {
    window: ByteWindow<S>,
    position: u64,
    remaining: u64,
    finished: bool,
}

impl<S: ByteSource> TarEntryStream<S> {
    /// Opens a reader over `entry`'s payload range.
    pub fn open(source: S, entry: &TarEntry) -> Result<Self, TarReadError> {
        let range = entry.data_range;
        let source_range =
            SourceRange::new(range.start, range.len).map_err(|error| TarReadError::InvalidRange(error.to_string()))?;
        let window = ByteWindow::new(source, 0, source_range).map_err(|error| TarReadError::Source(error.to_string()))?;
        Ok(Self { window, position: 0, remaining: range.len, finished: false })
    }

    /// Returns how many payload bytes this reader delivered.
    pub fn read_bytes(&self) -> u64 {
        self.position
    }

    /// Returns how many payload bytes remain.
    pub fn remaining_bytes(&self) -> u64 {
        self.remaining
    }

    /// Returns whether the declared payload length has been delivered.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Copies up to `out.len()` payload bytes.
    ///
    /// An empty destination yields `Produced(0)`. The final bytes are returned
    /// as `Produced`, and the following call reports `Finished`.
    pub fn read(&mut self, out: &mut [u8]) -> Result<StreamChunk, TarReadError> {
        if self.finished {
            return Ok(StreamChunk::Finished);
        }
        if out.is_empty() {
            return Ok(StreamChunk::Produced(0));
        }
        if self.remaining == 0 {
            self.finished = true;
            return Ok(StreamChunk::Finished);
        }
        let take = (out.len() as u64).min(self.remaining) as usize;
        let bytes_read = match self.window.read_at(self.position, &mut out[..take]) {
            Ok(ReadOutcome::Complete) => take,
            Ok(ReadOutcome::Partial { bytes_read }) => bytes_read as usize,
            Ok(ReadOutcome::Eof) => return Err(TarReadError::Source("payload ended before its declared size".to_owned())),
            Ok(ReadOutcome::Unavailable { range, reason }) => {
                let range =
                    ByteRange::new_allowing_empty(range.start, range.len).unwrap_or(ByteRange { start: range.start, len: 0 });
                return Err(TarReadError::NeedRange { range, reason });
            }
            Err(error) => return Err(TarReadError::Source(error.to_string())),
        };
        self.position += bytes_read as u64;
        self.remaining -= bytes_read as u64;
        if self.remaining == 0 {
            self.finished = true;
        }
        Ok(StreamChunk::Produced(bytes_read))
    }
}

fn read_block<S: ByteSource>(source: &S, offset: u64, length: u64) -> Result<Vec<u8>, TarIndexError> {
    if offset.checked_add(BLOCK_SIZE).is_none_or(|end| end > length) {
        return Err(TarIndexError::Truncated { offset });
    }
    let range = SourceRange::new(offset, BLOCK_SIZE).map_err(|error| TarIndexError::Source(error.to_string()))?;
    source.read_range(range).map_err(map_index_read_error)
}

fn read_metadata<S: ByteSource>(source: &S, offset: u64, size: u64, header_offset: u64) -> Result<Vec<u8>, TarIndexError> {
    if size > MAX_METADATA_BYTES {
        return Err(TarIndexError::MetadataTooLarge { offset: header_offset, size });
    }
    let range = SourceRange::new(offset, size).map_err(|error| TarIndexError::Source(error.to_string()))?;
    let mut bytes = source.read_range(range).map_err(map_index_read_error)?;
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    Ok(bytes)
}

pub(crate) fn padded_offset(end: u64) -> u64 {
    end + (BLOCK_SIZE - end % BLOCK_SIZE) % BLOCK_SIZE
}

fn map_index_read_error(error: ReadError) -> TarIndexError {
    match error {
        ReadError::Unavailable { range, reason } => TarIndexError::NeedRange {
            range: ByteRange::new_allowing_empty(range.start, range.len).unwrap_or(ByteRange { start: range.start, len: 0 }),
            reason,
        },
        ReadError::ShortRead { expected, actual } => {
            TarIndexError::Source(format!("short read: expected {expected}, got {actual}"))
        }
        ReadError::Eof => TarIndexError::Truncated { offset: 0 },
        ReadError::ConflictingRange { range } => {
            TarIndexError::Source(format!("conflicting bytes at {}..{}", range.start, range.start + range.len))
        }
        ReadError::Io(message) => TarIndexError::Source(message),
    }
}

fn map_read_error(error: ReadError) -> TarReadError {
    match error {
        ReadError::Unavailable { range, reason } => TarReadError::NeedRange {
            range: ByteRange::new_allowing_empty(range.start, range.len).unwrap_or(ByteRange { start: range.start, len: 0 }),
            reason,
        },
        ReadError::ShortRead { expected, actual } => {
            TarReadError::Source(format!("short read: expected {expected}, got {actual}"))
        }
        ReadError::Eof => TarReadError::InvalidRange("unexpected eof".to_owned()),
        ReadError::ConflictingRange { range } => {
            TarReadError::Source(format!("conflicting bytes at {}..{}", range.start, range.start + range.len))
        }
        ReadError::Io(message) => TarReadError::Source(message),
    }
}

fn error_field(error: acorn_core::RangeError) -> &'static str {
    match error {
        acorn_core::RangeError::Overflow => "data range overflow",
        acorn_core::RangeError::OutOfBounds => "data range out of bounds",
        acorn_core::RangeError::Empty => "data range is empty",
    }
}

#[derive(Default)]
pub(crate) struct PendingMetadata {
    pub(crate) long_name: Option<Vec<u8>>,
    pub(crate) long_link: Option<Vec<u8>>,
    pax: PaxOverrides,
}

impl PendingMetadata {
    pub(crate) fn has_values(&self) -> bool {
        self.long_name.is_some()
            || self.long_link.is_some()
            || self.pax.path.is_some()
            || self.pax.linkpath.is_some()
            || self.pax.size.is_some()
            || self.pax.modified_unix.is_some()
    }

    /// Resolves the member facts once the metadata records are complete.
    pub(crate) fn resolve(&mut self, header: &Header) -> (Vec<u8>, Vec<u8>, u64, i64) {
        let path = self.long_name.take().or_else(|| self.pax.path.take()).unwrap_or_else(|| header.path());
        let link_name = self.long_link.take().or_else(|| self.pax.linkpath.take()).unwrap_or_else(|| header.link_name.clone());
        let size = self.pax.size.take().unwrap_or(header.size);
        let modified_unix = self.pax.modified_unix.take().unwrap_or(header.modified_unix);
        (path, link_name, size, modified_unix)
    }

    pub(crate) fn apply_pax(&mut self, bytes: Vec<u8>, offset: u64) -> Result<(), TarIndexError> {
        for (key, value) in parse_pax_records(&bytes, offset)? {
            match key.as_slice() {
                b"path" => self.pax.path = Some(value.to_vec()),
                b"linkpath" => self.pax.linkpath = Some(value.to_vec()),
                b"size" => {
                    let text = std::str::from_utf8(&value).map_err(|_| TarIndexError::InvalidPaxRecord { offset })?;
                    let size = text.parse::<u64>().map_err(|_| TarIndexError::InvalidPaxRecord { offset })?;
                    self.pax.size = Some(size);
                }
                b"mtime" => {
                    let integer = value.split(|&byte| byte == b'.').next().unwrap_or(&value);
                    let text = std::str::from_utf8(integer).map_err(|_| TarIndexError::InvalidPaxRecord { offset })?;
                    let seconds = text.parse::<i64>().map_err(|_| TarIndexError::InvalidPaxRecord { offset })?;
                    self.pax.modified_unix = Some(seconds);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct PaxOverrides {
    path: Option<Vec<u8>>,
    linkpath: Option<Vec<u8>>,
    size: Option<u64>,
    modified_unix: Option<i64>,
}

/// One PAX `key=value` record, kept as raw bytes.
type PaxRecord = (Vec<u8>, Vec<u8>);

fn parse_pax_records(bytes: &[u8], offset: u64) -> Result<Vec<PaxRecord>, TarIndexError> {
    let mut records = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let remaining = &bytes[cursor..];
        let space = remaining.iter().position(|&byte| byte == b' ').ok_or(TarIndexError::InvalidPaxRecord { offset })?;
        let length = remaining[..space]
            .iter()
            .try_fold(0usize, |value, &byte| {
                value.checked_mul(10)?.checked_add(usize::from(byte).checked_sub(usize::from(b'0'))?)
            })
            .ok_or(TarIndexError::InvalidPaxRecord { offset })?;
        if length == 0 || length > remaining.len() || length < space + 3 {
            return Err(TarIndexError::InvalidPaxRecord { offset });
        }
        let record = &remaining[..length];
        if record.last() != Some(&b'\n') {
            return Err(TarIndexError::InvalidPaxRecord { offset });
        }
        let body = &record[space + 1..record.len() - 1];
        let equals = body.iter().position(|&byte| byte == b'=').ok_or(TarIndexError::InvalidPaxRecord { offset })?;
        records.push((body[..equals].to_vec(), body[equals + 1..].to_vec()));
        cursor += length;
    }
    Ok(records)
}

/// Borrowed byte source used by the tar-gzip container.
pub(crate) struct SliceSource<'a> {
    bytes: &'a [u8],
    identity: SourceIdentity,
}

impl<'a> SliceSource<'a> {
    pub(crate) fn new(label: impl Into<String>, bytes: &'a [u8]) -> Self {
        let identity = SourceIdentity::new(label).with_content_digest(content_digest_of(bytes));
        Self { bytes, identity }
    }
}

impl ByteSource for SliceSource<'_> {
    fn length(&self) -> Option<u64> {
        Some(self.bytes.len() as u64)
    }

    fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult {
        let offset = offset as usize;
        if offset >= self.bytes.len() {
            return Ok(ReadOutcome::Eof);
        }
        let available = self.bytes.len() - offset;
        let copied = destination.len().min(available);
        destination[..copied].copy_from_slice(&self.bytes[offset..offset + copied]);
        if copied == destination.len() {
            Ok(ReadOutcome::Complete)
        }
        else {
            Ok(ReadOutcome::Partial { bytes_read: copied as u64 })
        }
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::MEMORY
    }

    fn identity(&self) -> SourceIdentity {
        self.identity.clone()
    }
}
