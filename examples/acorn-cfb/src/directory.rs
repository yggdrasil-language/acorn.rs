use crate::error::{CfbError, Result};
use crate::fat::{FREE_SECTOR, NOSTREAM, END_OF_CHAIN};

/// Directory entry object type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryType {
    Invalid,
    Storage,
    Stream,
    LockBytes,
    Property,
    Root,
}

impl EntryType {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Storage,
            2 => Self::Stream,
            3 => Self::LockBytes,
            4 => Self::Property,
            5 => Self::Root,
            _ => Self::Invalid,
        }
    }
}

/// Parsed directory entry (128 bytes on disk).
#[derive(Debug, Clone)]
pub(crate) struct DirectoryEntry {
    pub name: String,
    pub entry_type: EntryType,
    pub left: u32,
    pub right: u32,
    pub child: u32,
    pub start_sector: u32,
    pub stream_size: u64,
}

impl DirectoryEntry {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 128 {
            return Err(CfbError::TooSmall {
                expected: 128,
                actual: raw.len(),
            });
        }

        let name_len = u16::from_le_bytes([raw[0x40], raw[0x41]]) as usize;
        let name = decode_entry_name(&raw[0..64], name_len)?;

        Ok(Self {
            name,
            entry_type: EntryType::from_raw(raw[0x42]),
            left: read_u32(raw, 0x44)?,
            right: read_u32(raw, 0x48)?,
            child: read_u32(raw, 0x4C)?,
            start_sector: read_u32(raw, 0x74)?,
            stream_size: u64::from_le_bytes([
                raw[0x78],
                raw[0x79],
                raw[0x7A],
                raw[0x7B],
                raw[0x7C],
                raw[0x7D],
                raw[0x7E],
                raw[0x7F],
            ]),
        })
    }
}

pub(crate) fn find_stream_entry<'a>(
    entries: &'a [DirectoryEntry],
    path: &str,
) -> Result<&'a DirectoryEntry> {
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return Err(CfbError::InvalidPath {
            path: path.to_string(),
        });
    }

    let mut current = 0usize;
    for segment in segments {
        let child_sid = find_child(entries, current, segment).ok_or_else(|| CfbError::StreamNotFound {
            path: path.to_string(),
        })?;
        current = child_sid;
    }

    let entry = &entries[current];
    if entry.entry_type != EntryType::Stream {
        return Err(CfbError::StreamNotFound {
            path: path.to_string(),
        });
    }
    Ok(entry)
}

fn find_child(entries: &[DirectoryEntry], parent_sid: usize, name: &str) -> Option<usize> {
    let parent = entries.get(parent_sid)?;
    if parent.child == NOSTREAM {
        return None;
    }
    find_in_tree(entries, parent.child as usize, name)
}

fn find_in_tree(entries: &[DirectoryEntry], sid: usize, name: &str) -> Option<usize> {
    let entry = entries.get(sid)?;
    let order = compare_entry_name(&entry.name, name);
    if order == Ordering::Equal {
        return Some(sid);
    }

    if order == Ordering::Less {
        if entry.right != NOSTREAM {
            if let Some(found) = find_in_tree(entries, entry.right as usize, name) {
                return Some(found);
            }
        }
        if entry.left != NOSTREAM {
            return find_in_tree(entries, entry.left as usize, name);
        }
        None
    } else if entry.left != NOSTREAM {
        if let Some(found) = find_in_tree(entries, entry.left as usize, name) {
            return Some(found);
        }
        if entry.right != NOSTREAM {
            return find_in_tree(entries, entry.right as usize, name);
        }
        None
    } else if entry.right != NOSTREAM {
        find_in_tree(entries, entry.right as usize, name)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ordering {
    Less,
    Equal,
    Greater,
}

fn compare_entry_name(left: &str, right: &str) -> Ordering {
    let left = left.trim_end_matches('\0');
    let right = right.trim_end_matches('\0');
    left.to_ascii_uppercase()
        .cmp(&right.to_ascii_uppercase())
        .then_with(|| left.len().cmp(&right.len()))
        .then_with(|| left.cmp(right))
        .into()
}

impl From<std::cmp::Ordering> for Ordering {
    fn from(value: std::cmp::Ordering) -> Self {
        match value {
            std::cmp::Ordering::Less => Self::Less,
            std::cmp::Ordering::Equal => Self::Equal,
            std::cmp::Ordering::Greater => Self::Greater,
        }
    }
}

fn decode_entry_name(raw: &[u8], name_len: usize) -> Result<String> {
    if name_len < 2 || name_len > 64 || name_len % 2 != 0 {
        return Err(CfbError::InvalidEntryName);
    }
    let utf16: Vec<u16> = raw[..name_len - 2]
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    String::from_utf16(&utf16).map_err(|_| CfbError::InvalidEntryName)
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ]))
}

pub(crate) fn load_directory(
    bytes: &[u8],
    sector_size: usize,
    first_dir_sector: u32,
    num_dir_sectors: u32,
    fat: &[u32],
) -> Result<Vec<DirectoryEntry>> {
    let mut entries = Vec::new();
    let mut sector_id = first_dir_sector;
    let mut sectors_read = 0u32;
    let max_sectors = num_dir_sectors.max(1);
    let entries_per_sector = sector_size / 128;

    while sector_id != FREE_SECTOR && sector_id != END_OF_CHAIN {
        if sectors_read >= max_sectors.saturating_add(8) {
            return Err(CfbError::ChainTooLong {
                context: "directory".to_string(),
            });
        }
        let sector = read_sector(bytes, sector_size, sector_id, fat)?;
        for index in 0..entries_per_sector {
            let offset = index * 128;
            entries.push(DirectoryEntry::parse(&sector[offset..offset + 128])?);
        }
        sector_id = fat
            .get(sector_id as usize)
            .copied()
            .ok_or_else(|| CfbError::UnexpectedEndOfChain {
                context: "directory".to_string(),
            })?;
        sectors_read += 1;
    }

    Ok(entries)
}

fn read_sector(bytes: &[u8], sector_size: usize, sector_id: u32, fat: &[u32]) -> Result<Vec<u8>> {
    let _ = fat;
    let start = (sector_id as usize + 1) * sector_size;
    let end = start + sector_size;
    if end > bytes.len() {
        return Err(CfbError::SectorOutOfRange {
            sector_id,
            max_sector: ((bytes.len() / sector_size).saturating_sub(1)) as u32,
        });
    }
    Ok(bytes[start..end].to_vec())
}
