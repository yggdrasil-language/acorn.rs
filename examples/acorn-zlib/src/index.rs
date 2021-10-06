//! Zlib layout indexing.

use acorn_core::{AddressSpaceId, LayoutGraph, LayoutNodeId, NodeState};
use acorn_probe::probe_memory;
use acorn_source::{ByteSource, ReadOutcome};

use crate::header::{ZlibHeader, ZlibHeaderError, parse_zlib_header};

/// Errors while indexing a zlib stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZlibIndexError {
    /// Input is not recognized as zlib.
    NotZlib,
    /// Header parsing failed.
    Header(ZlibHeaderError),
    /// Underlying layout graph rejected a range.
    Layout(String),
}

impl std::fmt::Display for ZlibIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotZlib => f.write_str("zlib signature not found"),
            Self::Header(error) => write!(f, "zlib header error: {error}"),
            Self::Layout(message) => write!(f, "layout error: {message}"),
        }
    }
}

impl std::error::Error for ZlibIndexError {}

/// Indexed zlib stream with layout graph nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZlibArchiveIndex {
    /// Primary file address space id.
    pub space: AddressSpaceId,
    /// Parsed zlib header.
    pub header: ZlibHeader,
    /// Layout graph for the file space.
    pub layout: LayoutGraph,
    /// Layout node for the zlib header region.
    pub header_node: LayoutNodeId,
    /// Layout node for the compressed member region.
    pub member_node: LayoutNodeId,
}

/// Indexes `source` when it contains a zlib stream.
pub fn index_zlib<S: ByteSource>(source: &S) -> Result<ZlibArchiveIndex, ZlibIndexError> {
    let bytes = read_all(source)?;
    index_zlib_bytes(bytes)
}

/// Indexes zlib bytes after probe confirmation.
pub fn index_zlib_bytes(bytes: Vec<u8>) -> Result<ZlibArchiveIndex, ZlibIndexError> {
    let hits = probe_memory("zlib-input", bytes.clone());
    if !hits.iter().any(|hit| hit.format_id == "zlib") {
        return Err(ZlibIndexError::NotZlib);
    }
    let header = parse_zlib_header(&bytes).map_err(ZlibIndexError::Header)?;
    let space = AddressSpaceId(0);
    let file_len = bytes.len() as u64;
    let mut layout = LayoutGraph::new(space, file_len);
    let header_node = layout
        .insert_node(
            NodeState::StructurallyValid,
            acorn_core::ByteRange::new(0, header.header_size as u64)
                .map_err(|error| ZlibIndexError::Layout(error.to_string()))?,
            "zlib header",
        )
        .map_err(|error| ZlibIndexError::Layout(error.to_string()))?;
    let member_node = layout
        .insert_node(NodeState::Indexed, header.member_range, "zlib member")
        .map_err(|error| ZlibIndexError::Layout(error.to_string()))?;
    Ok(ZlibArchiveIndex { space, header, layout, header_node, member_node })
}

fn read_all<S: ByteSource>(source: &S) -> Result<Vec<u8>, ZlibIndexError> {
    let length = source.length().ok_or(ZlibIndexError::NotZlib)? as usize;
    let mut buffer = vec![0u8; length];
    match source.read_at(0, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(ReadOutcome::Eof) => Err(ZlibIndexError::NotZlib),
        Ok(ReadOutcome::Unavailable { .. }) => Err(ZlibIndexError::NotZlib),
        Err(_) => Err(ZlibIndexError::NotZlib),
    }
}
