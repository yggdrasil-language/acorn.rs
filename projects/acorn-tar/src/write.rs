//! Tar writer for ustar members and GNU long names.

use std::fmt;

use crate::{
    format::{BLOCK_SIZE, build_header, split_ustar_path},
    model::{TarEntryDraft, TarEntryKind},
};

/// Tar writing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarWriteError {
    /// Payload bytes were supplied for a member kind that cannot carry data.
    DataOnNonRegularEntry {
        /// Member kind.
        kind: TarEntryKind,
    },
    /// A link target was supplied for a member kind that is not a link.
    LinkNameOnNonLinkEntry {
        /// Member kind.
        kind: TarEntryKind,
    },
}

impl fmt::Display for TarWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataOnNonRegularEntry { kind } => {
                write!(f, "tar member kind {kind:?} cannot carry payload bytes")
            }
            Self::LinkNameOnNonLinkEntry { kind } => {
                write!(f, "tar member kind {kind:?} cannot carry a link name")
            }
        }
    }
}

impl std::error::Error for TarWriteError {}

/// Builds a tar archive in memory.
#[derive(Debug, Clone, Default)]
pub struct TarWriter {
    entries: Vec<TarEntryDraft>,
}

impl TarWriter {
    /// Creates an empty writer.
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Adds one member.
    pub fn push(&mut self, entry: TarEntryDraft) {
        self.entries.push(entry);
    }

    /// Returns the members queued for writing.
    pub fn entries(&self) -> &[TarEntryDraft] {
        &self.entries
    }

    /// Writes every queued member plus the end-of-archive blocks.
    pub fn into_bytes(self) -> Result<Vec<u8>, TarWriteError> {
        write_tar(self.entries)
    }
}

/// Writes entries into a complete tar archive.
pub fn write_tar(entries: Vec<TarEntryDraft>) -> Result<Vec<u8>, TarWriteError> {
    let mut output = Vec::new();
    for entry in entries {
        write_entry(&mut output, &entry)?;
    }
    output.extend_from_slice(&[0u8; 1024]);
    Ok(output)
}

fn write_entry(output: &mut Vec<u8>, entry: &TarEntryDraft) -> Result<(), TarWriteError> {
    if !entry.kind.has_data() && !entry.data.is_empty() {
        return Err(TarWriteError::DataOnNonRegularEntry { kind: entry.kind });
    }
    if !entry.kind.has_link_name() && !entry.link_name.is_empty() {
        return Err(TarWriteError::LinkNameOnNonLinkEntry { kind: entry.kind });
    }

    let size = if entry.kind.has_data() { entry.data.len() as u64 } else { 0 };
    let (name, prefix) = match split_ustar_path(&entry.path) {
        Some((name, prefix)) => (name.to_vec(), prefix.to_vec()),
        None => {
            write_long_metadata(output, b'L', &entry.path);
            (entry.path[..entry.path.len().min(100)].to_vec(), Vec::new())
        }
    };
    let link_name = if entry.kind.has_link_name() && entry.link_name.len() > 100 {
        write_long_metadata(output, b'K', &entry.link_name);
        entry.link_name[..100].to_vec()
    }
    else {
        entry.link_name.clone()
    };

    let header = build_header(
        &name,
        &prefix,
        entry.mode,
        entry.uid,
        entry.gid,
        size,
        entry.modified_unix,
        entry.kind.typeflag(),
        &link_name,
    );
    output.extend_from_slice(&header);
    if entry.kind.has_data() {
        output.extend_from_slice(&entry.data);
    }
    pad_to_block(output);
    Ok(())
}

fn write_long_metadata(output: &mut Vec<u8>, typeflag: u8, value: &[u8]) {
    let header = build_header(b"././@LongLink", &[], 0, 0, 0, value.len() as u64 + 1, 0, typeflag, &[]);
    output.extend_from_slice(&header);
    output.extend_from_slice(value);
    output.push(0);
    pad_to_block(output);
}

fn pad_to_block(output: &mut Vec<u8>) {
    let remainder = output.len() as u64 % BLOCK_SIZE;
    if remainder != 0 {
        output.resize(output.len() + (BLOCK_SIZE - remainder) as usize, 0);
    }
}
