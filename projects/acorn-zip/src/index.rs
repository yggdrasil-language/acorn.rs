use acorn_core::{
    AddressSpaceId, ByteRange, LayoutGraph, LayoutNodeId, NeedRange, NodeState, ReferenceEdge, evidence_absolute,
};
use acorn_source::{ByteSource, MemorySource, ReadOutcome, SourceIdentity};

use crate::{
    entry::ZipMember,
    eocd::{EndOfCentralDirectory, read_u16_le, read_u32_le, scan_eocd_fields},
    zip64::{needs_zip64_size, parse_zip64_extra},
};

const CD_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CD_SIGNATURE_LEN: usize = 4;
/// Fixed fields after the 4-byte signature (APPNOTE central directory = 46 total).
const CD_FIXED_AFTER_SIG: usize = 42;
/// Central directory fixed prefix including signature.
const CD_ENTRY_PREFIX: usize = CD_SIGNATURE_LEN + CD_FIXED_AFTER_SIG;
/// Local file header fields up to the name length (APPNOTE local header = 30 bytes).
const LOCAL_HEADER_PREFIX: u64 = 30;
/// Smallest end-of-central-directory record (APPNOTE).
const EOCD_MIN_LEN: u64 = 22;
/// Largest interval searched for the end-of-central-directory record.
const EOCD_SEARCH_WINDOW: u64 = EOCD_MIN_LEN + 65_535;

/// Errors while indexing a ZIP archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipIndexError {
    /// Input is not recognized as a ZIP container.
    NotZip,
    /// End of central directory is missing or invalid.
    MissingEocd,
    /// Central directory range exceeds the source length.
    CentralDirectoryOutOfBounds,
    /// A central directory entry is truncated or malformed.
    MalformedEntry {
        /// Entry offset in the archive.
        offset: u64,
    },
    /// Underlying layout graph rejected a range.
    Layout(String),
    /// The source does not currently hold a required interval.
    ///
    /// The archive has not been read yet, so this is a range need rather than an
    /// invalid ZIP. The caller supplies the interval and indexes again.
    NeedRange {
        /// Missing interval in the archive file space.
        range: ByteRange,
    },
}

impl std::fmt::Display for ZipIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotZip => write!(f, "zip signature not found"),
            Self::MissingEocd => write!(f, "missing or invalid end of central directory"),
            Self::CentralDirectoryOutOfBounds => write!(f, "central directory out of bounds"),
            Self::MalformedEntry { offset } => {
                write!(f, "malformed central directory entry at offset {offset}")
            }
            Self::Layout(message) => write!(f, "layout error: {message}"),
            Self::NeedRange { range } => {
                write!(f, "source does not hold bytes for range {}..{}", range.start, range.start + range.len)
            }
        }
    }
}

impl std::error::Error for ZipIndexError {}

/// Indexed ZIP archive with layout graph nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipArchiveIndex {
    /// Source identity the member table was built from.
    pub identity: SourceIdentity,
    /// Primary file address space id.
    pub space: AddressSpaceId,
    /// Parsed EOCD record.
    pub eocd: EndOfCentralDirectory,
    /// Indexed members in central directory order.
    pub members: Vec<ZipMember>,
    /// Layout graph for the archive file space.
    pub layout: LayoutGraph,
    /// Layout node for the central directory region.
    pub central_directory_node: LayoutNodeId,
}

impl ZipIndexError {
    /// Builds the core range request for this failure, when it is one.
    ///
    /// A caller that owns a provider can ask for exactly the interval the archive could
    /// not supply, instead of reporting the archive as invalid.
    pub fn need_range(&self, space: AddressSpaceId, reason: &str) -> Option<NeedRange> {
        match self {
            Self::NeedRange { range } => Some(NeedRange { space, range: *range, reason: reason.to_string() }),
            _ => None,
        }
    }
}

impl ZipArchiveIndex {
    /// Whether this index may be reused for `current`.
    ///
    /// Reuse requires the same source label, version token, and content digest, so a
    /// source whose revision changed cannot keep serving the old member table.
    pub fn reuses(&self, current: &SourceIdentity) -> bool {
        self.identity.snapshot_reuses(current)
    }
}

/// Indexes `source` when it contains a ZIP central directory.
///
/// Only the end-of-central-directory record, the central directory, and each member's
/// local header are read. Payload bytes stay on the source until a member is decoded,
/// so indexing a large archive does not materialize it.
pub fn index_zip<S: ByteSource>(source: &S) -> Result<ZipArchiveIndex, ZipIndexError> {
    let identity = source.identity();
    let file_len = source.length().ok_or(ZipIndexError::NotZip)?;

    let Some((located, window_start)) = read_eocd(source, file_len)?
    else {
        return Err(missing_eocd_error(source, file_len)?);
    };
    let eocd = EndOfCentralDirectory { offset: located.offset + window_start, ..located };

    let space = AddressSpaceId(0);
    let mut layout = LayoutGraph::new(space, file_len);

    let (central_directory_node, members) = if eocd.entry_count == 0 {
        if eocd.central_directory_size != 0 {
            return Err(ZipIndexError::MissingEocd);
        }
        // An archive without members has no directory to read. The empty interval is
        // still recorded, because the record points at where it would begin.
        let empty = ByteRange::new_allowing_empty(eocd.central_directory_offset, 0)
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?;
        let node = layout
            .insert_node(NodeState::Indexed, empty, "ZIP empty central directory")
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?;
        (node, Vec::new())
    }
    else {
        let cd_range = eocd.central_directory_range().ok_or(ZipIndexError::MissingEocd)?;
        cd_range.fits_in(file_len).map_err(|error| ZipIndexError::Layout(error.to_string()))?;
        let node = layout
            .insert_node(NodeState::Indexed, cd_range, "ZIP central directory")
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?;
        // The declared directory must end before the record that points at it, so a
        // hostile size cannot ask for an allocation past the input.
        let directory_end = cd_range.end().map_err(|error| ZipIndexError::Layout(error.to_string()))?;
        if directory_end > eocd.offset {
            return Err(ZipIndexError::MissingEocd);
        }
        let directory = read_source_range(source, cd_range.start, cd_range.len)?;
        if read_u32_le(&directory, 0) != Some(CD_SIGNATURE) {
            return Err(ZipIndexError::MissingEocd);
        }
        // Member data lives before the directory, so its start is the end of the data region.
        let members = parse_central_directory(source, &directory, &eocd, &mut layout, space, cd_range.start)?;
        (node, members)
    };

    let _ = layout.insert_node(
        NodeState::Indexed,
        ByteRange::new(eocd.offset, eocd_record_len(eocd.comment_length))
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?,
        "ZIP end of central directory",
    );

    Ok(ZipArchiveIndex { identity, space, eocd, members, layout, central_directory_node })
}

/// Reads the end-of-central-directory record, starting from the shortest possible
/// tail and growing the window only while the record is not there yet.
///
/// A large archive therefore needs its last record plus the directory, not the whole
/// file. The returned offset is the start of the window the record was found in.
fn read_eocd<S: ByteSource>(source: &S, file_len: u64) -> Result<Option<(EndOfCentralDirectory, u64)>, ZipIndexError> {
    if file_len < EOCD_MIN_LEN {
        return Ok(None);
    }
    let cap = file_len.min(EOCD_SEARCH_WINDOW);
    let mut window = EOCD_MIN_LEN.min(cap);
    loop {
        let start = file_len - window;
        let bytes = read_source_range(source, start, window)?;
        if let Some(record) = scan_eocd_fields(&bytes) {
            return Ok(Some((record, start)));
        }
        let grown = window.saturating_mul(4).min(cap);
        if grown <= window {
            return Ok(None);
        }
        window = grown;
    }
}

/// Separates a not-yet-fetched archive from an input that is not a ZIP at all.
fn missing_eocd_error<S: ByteSource>(source: &S, file_len: u64) -> Result<ZipIndexError, ZipIndexError> {
    if file_len < CD_SIGNATURE_LEN as u64 {
        return Ok(ZipIndexError::NotZip);
    }
    let head = read_source_range(source, 0, CD_SIGNATURE_LEN as u64)?;
    Ok(if read_u32_le(&head, 0) == Some(LOCAL_SIGNATURE) { ZipIndexError::MissingEocd } else { ZipIndexError::NotZip })
}

/// Reads exactly `len` bytes at `start`, reporting an unfetched interval as a range need.
fn read_source_range<S: ByteSource>(source: &S, start: u64, len: u64) -> Result<Vec<u8>, ZipIndexError> {
    if len == 0 {
        return Ok(Vec::new());
    }
    start.checked_add(len).ok_or_else(|| ZipIndexError::Layout("source range overflow".to_string()))?;
    let length = usize::try_from(len).map_err(|_| ZipIndexError::Layout("source range is too large".to_string()))?;
    let mut buffer = vec![0u8; length];
    match source.read_at(start, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            // A random-access source that returns fewer bytes than the declared length
            // simply does not hold the rest of the interval yet, so the caller can
            // supply it instead of reading a shorter tree.
            let missing_start = start.saturating_add(bytes_read);
            let missing_len = len.saturating_sub(bytes_read);
            let range = ByteRange::new_allowing_empty(missing_start, missing_len)
                .map_err(|error| ZipIndexError::Layout(error.to_string()))?;
            Err(ZipIndexError::NeedRange { range })
        }
        Ok(ReadOutcome::Eof) => Err(ZipIndexError::NotZip),
        Ok(ReadOutcome::Unavailable { range, .. }) => {
            let range = ByteRange::new_allowing_empty(range.start, range.len)
                .map_err(|error| ZipIndexError::Layout(error.to_string()))?;
            Err(ZipIndexError::NeedRange { range })
        }
        Err(error) => Err(ZipIndexError::Layout(error.to_string())),
    }
}

fn eocd_record_len(comment_length: u16) -> u64 {
    22 + comment_length as u64
}

fn parse_central_directory<S: ByteSource>(
    source: &S,
    directory: &[u8],
    eocd: &EndOfCentralDirectory,
    layout: &mut LayoutGraph,
    space: AddressSpaceId,
    data_end: u64,
) -> Result<Vec<ZipMember>, ZipIndexError> {
    let base = eocd.central_directory_offset;
    let end = directory.len();
    let mut offset: usize = 0;
    let mut members = Vec::new();
    for _ in 0..eocd.entry_count {
        let entry_offset = base + offset as u64;
        if read_u32_le(directory, offset) != Some(CD_SIGNATURE) {
            return Err(ZipIndexError::MalformedEntry { offset: entry_offset });
        }
        let version_needed = read_u16_le(directory, offset + 6).unwrap_or(0);
        let general_purpose_bit_flag = read_u16_le(directory, offset + 8).unwrap_or(0);
        let compression_method = read_u16_le(directory, offset + 10).unwrap_or(0);
        let crc32 = read_u32_le(directory, offset + 16).unwrap_or(0);
        let compressed32 = read_u32_le(directory, offset + 20).unwrap_or(0);
        let uncompressed32 = read_u32_le(directory, offset + 24).unwrap_or(0);
        let name_length = read_u16_le(directory, offset + 28).unwrap_or(0) as usize;
        let extra_length = read_u16_le(directory, offset + 30).unwrap_or(0) as usize;
        let comment_length = read_u16_le(directory, offset + 32).unwrap_or(0) as usize;
        let local_offset32 = read_u32_le(directory, offset + 42).unwrap_or(0);

        let needs_uncompressed = needs_zip64_size(uncompressed32);
        let needs_compressed = needs_zip64_size(compressed32);
        let needs_local_offset = needs_zip64_size(local_offset32);

        let name_start = offset + CD_ENTRY_PREFIX;
        let name_end = name_start + name_length;
        if name_end > end {
            return Err(ZipIndexError::MalformedEntry { offset: entry_offset });
        }
        let path = String::from_utf8_lossy(&directory[name_start..name_end]).into_owned();

        let extra_start = name_end;
        let zip64 = if needs_uncompressed || needs_compressed || needs_local_offset {
            parse_zip64_extra(
                directory,
                extra_start,
                extra_length,
                needs_uncompressed,
                needs_compressed,
                needs_local_offset,
                uncompressed32,
                compressed32,
                local_offset32,
            )
            .ok_or(ZipIndexError::MalformedEntry { offset: entry_offset })?
        }
        else {
            crate::zip64::Zip64Fields {
                uncompressed_size: uncompressed32 as u64,
                compressed_size: compressed32 as u64,
                local_header_offset: local_offset32 as u64,
            }
        };

        let entry_len = CD_ENTRY_PREFIX + name_length + extra_length + comment_length;
        let entry_range = ByteRange::new(entry_offset, entry_len as u64)
            .map_err(|_| ZipIndexError::MalformedEntry { offset: entry_offset })?;

        let member_node = layout
            .insert_node(NodeState::Indexed, entry_range, format!("ZIP member {path}"))
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?;

        let (data_range, payload_range) =
            member_payload_range(source, zip64.local_header_offset, zip64.compressed_size, data_end)?;

        layout.add_edge(ReferenceEdge {
            from: member_node,
            to: member_node,
            evidence: evidence_absolute(space, "local header", data_range),
        });

        members.push(ZipMember {
            path,
            version_needed,
            general_purpose_bit_flag,
            compression_method,
            crc32,
            local_header_offset: zip64.local_header_offset,
            compressed_size: zip64.compressed_size,
            uncompressed_size: zip64.uncompressed_size,
            data_range,
            payload_range,
        });

        offset += entry_len;
    }
    Ok(members)
}

fn member_payload_range<S: ByteSource>(
    source: &S,
    local_header_offset: u64,
    compressed_size: u64,
    data_end: u64,
) -> Result<(ByteRange, ByteRange), ZipIndexError> {
    let header_end = local_header_offset
        .checked_add(LOCAL_HEADER_PREFIX)
        .ok_or(ZipIndexError::MalformedEntry { offset: local_header_offset })?;
    if header_end > data_end {
        return Err(ZipIndexError::MalformedEntry { offset: local_header_offset });
    }
    let prefix = read_source_range(source, local_header_offset, LOCAL_HEADER_PREFIX)?;
    if read_u32_le(&prefix, 0) != Some(LOCAL_SIGNATURE) {
        return Err(ZipIndexError::MalformedEntry { offset: local_header_offset });
    }
    let name_length = read_u16_le(&prefix, 26).unwrap_or(0) as u64;
    let extra_length = read_u16_le(&prefix, 28).unwrap_or(0) as u64;
    let header_len = LOCAL_HEADER_PREFIX
        .checked_add(name_length)
        .and_then(|len| len.checked_add(extra_length))
        .ok_or(ZipIndexError::MalformedEntry { offset: local_header_offset })?;
    let payload_start =
        local_header_offset.checked_add(header_len).ok_or(ZipIndexError::MalformedEntry { offset: local_header_offset })?;
    let payload_end =
        payload_start.checked_add(compressed_size).ok_or(ZipIndexError::MalformedEntry { offset: local_header_offset })?;
    if payload_end > data_end {
        return Err(ZipIndexError::MalformedEntry { offset: local_header_offset });
    }
    let data_range = ByteRange::new(local_header_offset, header_len + compressed_size)
        .map_err(|_| ZipIndexError::MalformedEntry { offset: local_header_offset })?;
    // Empty stored files and directory entries have a zero-length payload.
    let payload_range = ByteRange::new_allowing_empty(payload_start, compressed_size)
        .map_err(|_| ZipIndexError::MalformedEntry { offset: local_header_offset })?;
    Ok((data_range, payload_range))
}

/// Indexes in-memory archive bytes.
pub fn index_zip_bytes(label: impl Into<String>, bytes: Vec<u8>) -> Result<ZipArchiveIndex, ZipIndexError> {
    let source = MemorySource::from_bytes(label, bytes);
    index_zip(&source)
}
