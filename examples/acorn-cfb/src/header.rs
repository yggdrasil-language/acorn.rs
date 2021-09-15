use crate::error::{CfbError, Result};

/// Magic signature at the start of a compound file header.
pub const COMPOUND_FILE_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

const HEADER_SIZE: usize = 512;

/// Parsed CFB header (512 bytes).
#[derive(Debug, Clone)]
pub(crate) struct Header {
    pub sector_shift: u16,
    pub mini_sector_shift: u16,
    pub num_dir_sectors: u32,
    pub num_fat_sectors: u32,
    pub first_dir_sector: u32,
    pub mini_stream_cutoff: u32,
    pub first_mini_fat_sector: u32,
    pub num_mini_fat_sectors: u32,
    pub first_difat_sector: u32,
    pub num_difat_sectors: u32,
    pub difat: [u32; 109],
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(CfbError::TooSmall {
                expected: HEADER_SIZE,
                actual: bytes.len(),
            });
        }
        if bytes[0..8] != COMPOUND_FILE_SIGNATURE {
            return Err(CfbError::InvalidSignature);
        }

        let sector_shift = u16::from_le_bytes([bytes[0x1E], bytes[0x1F]]);
        let mini_sector_shift = u16::from_le_bytes([bytes[0x20], bytes[0x21]]);
        if sector_shift != 9 && sector_shift != 12 {
            return Err(CfbError::InvalidSectorShift { sector_shift });
        }
        if mini_sector_shift != 6 {
            return Err(CfbError::InvalidMiniSectorShift { mini_sector_shift });
        }

        let mut difat = [0u32; 109];
        for (index, slot) in difat.iter_mut().enumerate() {
            let offset = 0x4C + index * 4;
            *slot = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
        }

        Ok(Self {
            sector_shift,
            mini_sector_shift,
            num_dir_sectors: read_u32(bytes, 0x28)?,
            num_fat_sectors: read_u32(bytes, 0x2C)?,
            first_dir_sector: read_u32(bytes, 0x30)?,
            mini_stream_cutoff: read_u32(bytes, 0x38)?,
            first_mini_fat_sector: read_u32(bytes, 0x3C)?,
            num_mini_fat_sectors: read_u32(bytes, 0x40)?,
            first_difat_sector: read_u32(bytes, 0x44)?,
            num_difat_sectors: read_u32(bytes, 0x48)?,
            difat,
        })
    }

    pub fn sector_size(&self) -> usize {
        1usize << self.sector_shift
    }

    pub fn mini_sector_size(&self) -> usize {
        1usize << self.mini_sector_shift
    }
}

/// Returns whether `bytes` begins with the compound file signature.
pub fn is_compound_file(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[0..8] == COMPOUND_FILE_SIGNATURE
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    if offset + 4 > bytes.len() {
        return Err(CfbError::TooSmall {
            expected: offset + 4,
            actual: bytes.len(),
        });
    }
    Ok(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}
