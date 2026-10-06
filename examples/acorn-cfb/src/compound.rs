use std::io::{Read, Seek};

use crate::directory::{find_stream_entry, load_directory, EntryType};
use crate::error::{CfbError, Result};
use crate::fat::{load_fat, load_mini_fat, read_stream};
use crate::header::{Header, COMPOUND_FILE_SIGNATURE};

const DEFAULT_MAX_STREAM_BYTES: u64 = 64 * 1024 * 1024;

/// In-memory compound file reader.
#[derive(Debug, Clone)]
pub struct CompoundFile {
    bytes: Vec<u8>,
    sector_size: usize,
    mini_sector_size: usize,
    mini_stream_cutoff: u32,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    directory: Vec<crate::directory::DirectoryEntry>,
    mini_stream_start: u32,
    mini_container_size: u64,
    max_stream_bytes: u64,
}

impl CompoundFile {
    /// Opens a compound file from an in-memory byte slice.
    pub fn open(bytes: impl Into<Vec<u8>>) -> Result<Self> {
        Self::open_with_limit(bytes, DEFAULT_MAX_STREAM_BYTES)
    }

    /// Opens a compound file with a custom maximum stream read size.
    pub fn open_with_limit(bytes: impl Into<Vec<u8>>, max_stream_bytes: u64) -> Result<Self> {
        let bytes = bytes.into();
        let header = Header::parse(&bytes)?;
        let sector_size = header.sector_size();
        let mini_sector_size = header.mini_sector_size();
        let fat = load_fat(&bytes, &header)?;
        let mini_fat = load_mini_fat(&bytes, &header, &fat)?;
        let directory = load_directory(
            &bytes,
            sector_size,
            header.first_dir_sector,
            header.num_dir_sectors,
            &fat,
        )?;
        let root = directory
            .first()
            .ok_or_else(|| CfbError::UnexpectedEndOfChain {
                context: "root directory entry".to_string(),
            })?;
        if root.entry_type != EntryType::Root {
            return Err(CfbError::UnexpectedEndOfChain {
                context: "root directory entry".to_string(),
            });
        }

        let mini_container_size = root.stream_size;

        Ok(Self {
            mini_stream_start: root.start_sector,
            mini_container_size,
            mini_stream_cutoff: header.mini_stream_cutoff,
            sector_size,
            mini_sector_size,
            fat,
            mini_fat,
            directory,
            bytes,
            max_stream_bytes,
        })
    }

    /// Opens a compound file from any reader (loads entire file into memory).
    pub fn open_reader(mut reader: impl Read + Seek) -> Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Self::open(bytes)
    }

    /// Returns whether `bytes` begins with the compound file signature.
    pub fn is_compound_file(bytes: &[u8]) -> bool {
        bytes.len() >= COMPOUND_FILE_SIGNATURE.len()
            && bytes[0..COMPOUND_FILE_SIGNATURE.len()] == COMPOUND_FILE_SIGNATURE
    }

    /// Reads an entire stream by absolute path (for example `/WordDocument`).
    pub fn read_stream(&self, path: &str) -> Result<Vec<u8>> {
        let entry = find_stream_entry(&self.directory, path)?;
        if entry.stream_size > self.max_stream_bytes {
            return Err(CfbError::StreamTooLarge {
                path: path.to_string(),
                size: entry.stream_size,
                limit: self.max_stream_bytes,
            });
        }
        read_stream(
            &self.bytes,
            self.sector_size,
            self.mini_sector_size,
            self.mini_stream_cutoff,
            &self.fat,
            &self.mini_fat,
            self.mini_stream_start,
            self.mini_container_size,
            entry.start_sector,
            entry.stream_size,
            path,
        )
    }
}
