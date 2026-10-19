//! Gzip layout indexing.

use acorn_core::{AddressSpaceId, LayoutGraph, LayoutNodeId, NodeState};
use acorn_probe::probe_memory;
use acorn_source::{ByteSource, ReadOutcome};

use crate::header::{GzipHeader, GzipHeaderError, parse_gzip_header};

/// Errors while indexing a gzip file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GzipIndexError {
    /// Input is not recognized as gzip.
    NotGzip,
    /// Header parsing failed.
    Header(GzipHeaderError),
    /// Underlying layout graph rejected a range.
    Layout(String),
}

impl std::fmt::Display for GzipIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotGzip => f.write_str("gzip signature not found"),
            Self::Header(error) => write!(f, "gzip header error: {error}"),
            Self::Layout(message) => write!(f, "layout error: {message}"),
        }
    }
}

impl std::error::Error for GzipIndexError {}

/// Indexed gzip file with layout graph nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GzipArchiveIndex {
    /// Primary file address space id.
    pub space: AddressSpaceId,
    /// Parsed gzip header.
    pub header: GzipHeader,
    /// Layout graph for the file space.
    pub layout: LayoutGraph,
    /// Layout node for the gzip header region.
    pub header_node: LayoutNodeId,
    /// Layout node for the compressed member region.
    pub member_node: LayoutNodeId,
}

/// Indexes `source` when it contains a gzip member.
pub fn index_gzip<S: ByteSource>(source: &S) -> Result<GzipArchiveIndex, GzipIndexError> {
    let bytes = read_all(source)?;
    index_gzip_bytes(bytes)
}

/// Indexes gzip bytes after probe confirmation.
pub fn index_gzip_bytes(bytes: Vec<u8>) -> Result<GzipArchiveIndex, GzipIndexError> {
    let hits = probe_memory("gzip-input", bytes.clone());
    if !hits.iter().any(|hit| hit.format_id == "gzip") {
        return Err(GzipIndexError::NotGzip);
    }
    let header = parse_gzip_header(&bytes).map_err(GzipIndexError::Header)?;
    let space = AddressSpaceId(0);
    let file_len = bytes.len() as u64;
    let mut layout = LayoutGraph::new(space, file_len);
    let header_node = layout
        .insert_node(
            NodeState::StructurallyValid,
            acorn_core::ByteRange::new(0, header.header_size as u64)
                .map_err(|error| GzipIndexError::Layout(error.to_string()))?,
            "gzip header",
        )
        .map_err(|error| GzipIndexError::Layout(error.to_string()))?;
    let member_node = layout
        .insert_node(NodeState::Indexed, header.member_range, "gzip member")
        .map_err(|error| GzipIndexError::Layout(error.to_string()))?;
    Ok(GzipArchiveIndex { space, header, layout, header_node, member_node })
}

fn read_all<S: ByteSource>(source: &S) -> Result<Vec<u8>, GzipIndexError> {
    let length = source.length().ok_or(GzipIndexError::NotGzip)? as usize;
    let mut buffer = vec![0u8; length];
    match source.read_at(0, &mut buffer) {
        Ok(ReadOutcome::Complete) => Ok(buffer),
        Ok(ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(ReadOutcome::Eof) => Err(GzipIndexError::NotGzip),
        Ok(ReadOutcome::Unavailable { .. }) => Err(GzipIndexError::NotGzip),
        Err(_) => Err(GzipIndexError::NotGzip),
    }
}
