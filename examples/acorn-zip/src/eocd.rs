use acorn_core::ByteRange;

use crate::zip64::{needs_zip64_eocd, resolve_zip64_eocd};

const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const EOCD_MIN_SIZE: usize = 22;
const MAX_COMMENT: usize = 65_535;

pub(crate) fn read_u64_le(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset + 8)?;
    let mut array = [0u8; 8];
    array.copy_from_slice(slice);
    Some(u64::from_le_bytes(array))
}

/// Parsed end-of-central-directory record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndOfCentralDirectory {
    /// Offset of this EOCD record.
    pub offset: u64,
    /// Number of central directory entries.
    pub entry_count: u64,
    /// Size of the central directory in bytes.
    pub central_directory_size: u64,
    /// Offset of the central directory from the start of the file.
    pub central_directory_offset: u64,
    /// Trailing archive comment length.
    pub comment_length: u16,
    /// Whether ZIP64 EOCD fields were used.
    pub zip64: bool,
}

/// Locates and parses the EOCD near the end of `bytes`.
pub fn find_eocd(bytes: &[u8]) -> Option<EndOfCentralDirectory> {
    if bytes.len() < EOCD_MIN_SIZE {
        return None;
    }
    let search_start = bytes.len().saturating_sub(EOCD_MIN_SIZE + MAX_COMMENT);
    for offset in (search_start..bytes.len()).rev() {
        if read_u32_le(bytes, offset) != Some(EOCD_SIGNATURE) {
            continue;
        }
        if let Some(record) = parse_eocd(bytes, offset) {
            return Some(record);
        }
    }
    None
}

fn parse_eocd(bytes: &[u8], offset: usize) -> Option<EndOfCentralDirectory> {
    if offset + EOCD_MIN_SIZE > bytes.len() {
        return None;
    }
    let comment_length = read_u16_le(bytes, offset + 20)? as usize;
    let record_end = offset + EOCD_MIN_SIZE + comment_length;
    if record_end > bytes.len() {
        return None;
    }

    let entry_count16 = read_u16_le(bytes, offset + 10)?;
    let cd_size32 = read_u32_le(bytes, offset + 12)?;
    let cd_offset32 = read_u32_le(bytes, offset + 16)?;

    let (entry_count, central_directory_size, central_directory_offset, zip64) =
        if needs_zip64_eocd(entry_count16, cd_size32, cd_offset32) {
            let (count, size, cd_offset) = resolve_zip64_eocd(bytes, offset)?;
            (count, size, cd_offset, true)
        } else {
            (
                entry_count16 as u64,
                cd_size32 as u64,
                cd_offset32 as u64,
                false,
            )
        };

    Some(EndOfCentralDirectory {
        offset: offset as u64,
        entry_count,
        central_directory_size,
        central_directory_offset,
        comment_length: comment_length as u16,
        zip64,
    })
}

/// Byte range covering the central directory.
impl EndOfCentralDirectory {
    pub fn central_directory_range(&self) -> Option<ByteRange> {
        ByteRange::new(self.central_directory_offset, self.central_directory_size)
        .ok()
    }
}

pub(crate) fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

pub(crate) fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}
