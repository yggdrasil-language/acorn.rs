use acorn_core::{
    evidence_absolute, AddressSpaceId, ByteRange, LayoutGraph, LayoutNodeId, NodeState, ReferenceEdge,
};
use acorn_probe::probe_memory;
use acorn_source::{ByteSource, MemorySource, ReadOutcome};

use crate::eocd::{find_eocd, read_u16_le, read_u32_le, EndOfCentralDirectory};
use crate::entry::ZipMember;
use crate::zip64::{needs_zip64_size, parse_zip64_extra};

const CD_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CD_SIGNATURE_LEN: usize = 4;
const CD_FIXED: usize = 46;
const CD_ENTRY_PREFIX: usize = CD_SIGNATURE_LEN + CD_FIXED;

/// Errors while indexing a ZIP archive.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipIndexError {
    /// Input is not recognized as a ZIP container.
    #[error("zip signature not found")]
    NotZip,
    /// End of central directory is missing or invalid.
    #[error("missing or invalid end of central directory")]
    MissingEocd,
    /// Central directory range exceeds the source length.
    #[error("central directory out of bounds")]
    CentralDirectoryOutOfBounds,
    /// A central directory entry is truncated or malformed.
    #[error("malformed central directory entry at offset {offset}")]
    MalformedEntry {
        /// Entry offset in the archive.
        offset: u64,
    },
    /// Underlying layout graph rejected a range.
    #[error("layout error: {0}")]
    Layout(String),
}

/// Indexed ZIP archive with layout graph nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipArchiveIndex {
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

/// Indexes `source` when it contains a ZIP central directory.
pub fn index_zip<S: ByteSource>(source: &S) -> Result<ZipArchiveIndex, ZipIndexError> {
    let bytes = read_all(source)?;
    let hits = probe_memory("zip-input", bytes.clone());
    if !hits.iter().any(|hit| hit.format_id == "zip") {
        return Err(ZipIndexError::NotZip);
    }
    index_bytes(bytes)
}

fn read_all<S: ByteSource>(source: &S) -> Result<Vec<u8>, ZipIndexError> {
    let length = source.length().ok_or(ZipIndexError::NotZip)? as usize;
    let mut buffer = vec![0u8; length];
    match source.read_at(0, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(ReadOutcome::Eof) => Err(ZipIndexError::NotZip),
        Ok(ReadOutcome::Unavailable { .. }) => Err(ZipIndexError::NotZip),
        Err(_) => Err(ZipIndexError::NotZip),
    }
}

fn index_bytes(bytes: Vec<u8>) -> Result<ZipArchiveIndex, ZipIndexError> {
    let eocd = find_eocd(&bytes).ok_or(ZipIndexError::MissingEocd)?;
    let space = AddressSpaceId(0);
    let file_len = bytes.len() as u64;
    let mut layout = LayoutGraph::new(space, file_len);

    let cd_range = eocd
        .central_directory_range()
        .ok_or(ZipIndexError::MissingEocd)?;
    cd_range
        .fits_in(file_len)
        .map_err(|error| ZipIndexError::Layout(error.to_string()))?;

    let central_directory_node = layout
        .insert_node(NodeState::Indexed, cd_range, "ZIP central directory")
        .map_err(|error| ZipIndexError::Layout(error.to_string()))?;

    let members = parse_central_directory(&bytes, &eocd, &mut layout, space)?;

    let _ = layout.insert_node(
        NodeState::Indexed,
        ByteRange::new(eocd.offset, eocd_record_len(eocd.comment_length))
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?,
        "ZIP end of central directory",
    );

    Ok(ZipArchiveIndex {
        space,
        eocd,
        members,
        layout,
        central_directory_node,
    })
}

fn eocd_record_len(comment_length: u16) -> u64 {
    22 + comment_length as u64
}

fn parse_central_directory(
    bytes: &[u8],
    eocd: &EndOfCentralDirectory,
    layout: &mut LayoutGraph,
    space: AddressSpaceId,
) -> Result<Vec<ZipMember>, ZipIndexError> {
    let mut offset = eocd.central_directory_offset as usize;
    let end = offset
        .checked_add(eocd.central_directory_size as usize)
        .ok_or(ZipIndexError::CentralDirectoryOutOfBounds)?;
    if end > bytes.len() {
        return Err(ZipIndexError::CentralDirectoryOutOfBounds);
    }

    let mut members = Vec::new();
    for _ in 0..eocd.entry_count {
        if read_u32_le(bytes, offset) != Some(CD_SIGNATURE) {
            return Err(ZipIndexError::MalformedEntry {
                offset: offset as u64,
            });
        }
        let compression_method = read_u16_le(bytes, offset + 10).unwrap_or(0);
        let compressed32 = read_u32_le(bytes, offset + 20).unwrap_or(0);
        let uncompressed32 = read_u32_le(bytes, offset + 24).unwrap_or(0);
        let name_length = read_u16_le(bytes, offset + 28).unwrap_or(0) as usize;
        let extra_length = read_u16_le(bytes, offset + 30).unwrap_or(0) as usize;
        let comment_length = read_u16_le(bytes, offset + 32).unwrap_or(0) as usize;
        let local_offset32 = read_u32_le(bytes, offset + 42).unwrap_or(0);

        let needs_uncompressed = needs_zip64_size(uncompressed32);
        let needs_compressed = needs_zip64_size(compressed32);
        let needs_local_offset = needs_zip64_size(local_offset32);

        let name_start = offset + CD_ENTRY_PREFIX;
        let name_end = name_start + name_length;
        if name_end > end {
            return Err(ZipIndexError::MalformedEntry {
                offset: offset as u64,
            });
        }
        let path = String::from_utf8_lossy(&bytes[name_start..name_end]).into_owned();

        let extra_start = name_end;
        let zip64 = if needs_uncompressed || needs_compressed || needs_local_offset {
            parse_zip64_extra(
                bytes,
                extra_start,
                extra_length,
                needs_uncompressed,
                needs_compressed,
                needs_local_offset,
            )
            .ok_or(ZipIndexError::MalformedEntry {
                offset: offset as u64,
            })?
        } else {
            crate::zip64::Zip64Fields {
                uncompressed_size: uncompressed32 as u64,
                compressed_size: compressed32 as u64,
                local_header_offset: local_offset32 as u64,
            }
        };

        let entry_len = CD_ENTRY_PREFIX + name_length + extra_length + comment_length;
        let entry_range = ByteRange::new(offset as u64, entry_len as u64).map_err(|_| {
            ZipIndexError::MalformedEntry {
                offset: offset as u64,
            }
        })?;

        let member_node = layout
            .insert_node(NodeState::Indexed, entry_range, format!("ZIP member {}", path))
            .map_err(|error| ZipIndexError::Layout(error.to_string()))?;

        let (data_range, payload_range) = member_payload_range(
            bytes,
            zip64.local_header_offset,
            zip64.compressed_size,
        )?;

        layout.add_edge(ReferenceEdge {
            from: member_node,
            to: member_node,
            evidence: evidence_absolute(space, "local header", data_range),
        });

        members.push(ZipMember {
            path,
            local_header_offset: zip64.local_header_offset,
            compressed_size: zip64.compressed_size,
            uncompressed_size: zip64.uncompressed_size,
            compression_method,
            data_range,
            payload_range,
        });

        offset += entry_len;
    }
    Ok(members)
}

fn member_payload_range(
    bytes: &[u8],
    local_header_offset: u64,
    compressed_size: u64,
) -> Result<(ByteRange, ByteRange), ZipIndexError> {
    let offset = local_header_offset as usize;
    if read_u32_le(bytes, offset) != Some(LOCAL_SIGNATURE) {
        return Err(ZipIndexError::MalformedEntry {
            offset: local_header_offset,
        });
    }
    let name_length = read_u16_le(bytes, offset + 26).unwrap_or(0) as usize;
    let extra_length = read_u16_le(bytes, offset + 28).unwrap_or(0) as usize;
    let header_len = 30 + name_length + extra_length;
    let payload_start = offset
        .checked_add(header_len)
        .ok_or(ZipIndexError::MalformedEntry {
            offset: local_header_offset,
        })?;
    let payload_end = payload_start
        .checked_add(compressed_size as usize)
        .ok_or(ZipIndexError::MalformedEntry {
            offset: local_header_offset,
        })?;
    if payload_end > bytes.len() {
        return Err(ZipIndexError::MalformedEntry {
            offset: local_header_offset,
        });
    }
    let total_len = header_len as u64 + compressed_size;
    let data_range = ByteRange::new(local_header_offset, total_len).map_err(|_| {
        ZipIndexError::MalformedEntry {
            offset: local_header_offset,
        }
    })?;
    let payload_range = ByteRange::new(payload_start as u64, compressed_size).map_err(|_| {
        ZipIndexError::MalformedEntry {
            offset: local_header_offset,
        }
    })?;
    Ok((data_range, payload_range))
}

/// Indexes in-memory archive bytes.
pub fn index_zip_bytes(label: impl Into<String>, bytes: Vec<u8>) -> Result<ZipArchiveIndex, ZipIndexError> {
    let source = MemorySource::from_bytes(label, bytes);
    index_zip(&source)
}
