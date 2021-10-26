//! Tar-gzip container composition.

use std::fmt;

use acorn_codec::{
    CodecError, CodecLimits,
    compression::{Gzip, deflate::DeflateLevel},
};

use crate::{
    model::{TarArchive, TarEntry},
    read::{SliceSource, TarIndexError, index_tar},
};

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
