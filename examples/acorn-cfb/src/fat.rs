use crate::error::{CfbError, Result};
use crate::header::Header;

pub(crate) const FREE_SECTOR: u32 = 0xFFFF_FFFF;
pub(crate) const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
pub(crate) const FAT_SECTOR: u32 = 0xFFFF_FFFD;
pub(crate) const DIFAT_SECTOR: u32 = 0xFFFF_FFFC;
pub(crate) const NOSTREAM: u32 = 0xFFFF_FFFF;

const MAX_CHAIN_STEPS: usize = 1_000_000;

pub(crate) fn load_fat(bytes: &[u8], header: &Header) -> Result<Vec<u32>> {
    let sector_size = header.sector_size();
    let entries_per_sector = sector_size / 4;
    let mut fat_sector_ids = Vec::with_capacity(header.num_fat_sectors as usize);

    for sector_id in header.difat {
        if sector_id == FREE_SECTOR {
            break;
        }
        fat_sector_ids.push(sector_id);
    }

    if header.num_difat_sectors > 0 && header.first_difat_sector != FREE_SECTOR {
        let mut difat_sector = header.first_difat_sector;
        let mut difat_sectors_read = 0u32;
        while difat_sector != END_OF_CHAIN {
            if difat_sectors_read >= header.num_difat_sectors.saturating_add(4) {
                return Err(CfbError::ChainTooLong {
                    context: "DIFAT".to_string(),
                });
            }
            let sector = read_sector(bytes, sector_size, difat_sector)?;
            let usable = entries_per_sector.saturating_sub(1);
            for index in 0..usable {
                let offset = index * 4;
                let next = u32::from_le_bytes([
                    sector[offset],
                    sector[offset + 1],
                    sector[offset + 2],
                    sector[offset + 3],
                ]);
                if next == FREE_SECTOR {
                    break;
                }
                fat_sector_ids.push(next);
            }
            let link_offset = usable * 4;
            difat_sector = u32::from_le_bytes([
                sector[link_offset],
                sector[link_offset + 1],
                sector[link_offset + 2],
                sector[link_offset + 3],
            ]);
            difat_sectors_read += 1;
        }
    }

    let mut fat = Vec::with_capacity(fat_sector_ids.len() * entries_per_sector);
    for sector_id in fat_sector_ids {
        let sector = read_sector(bytes, sector_size, sector_id)?;
        for chunk in sector.chunks_exact(4) {
            fat.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
    }
    Ok(fat)
}

pub(crate) fn load_mini_fat(bytes: &[u8], header: &Header, fat: &[u32]) -> Result<Vec<u32>> {
    let sector_size = header.sector_size();
    let mini_entries_per_sector = sector_size / 4;
    let mut mini_fat = Vec::with_capacity(header.num_mini_fat_sectors as usize * mini_entries_per_sector);
    let mut sector_id = header.first_mini_fat_sector;
    let mut sectors_read = 0u32;

    while sector_id != END_OF_CHAIN && sector_id != FREE_SECTOR {
        if sectors_read >= header.num_mini_fat_sectors.saturating_add(8) {
            return Err(CfbError::ChainTooLong {
                context: "mini FAT".to_string(),
            });
        }
        let sector = read_sector(bytes, sector_size, sector_id)?;
        for chunk in sector.chunks_exact(4) {
            mini_fat.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        sector_id = fat
            .get(sector_id as usize)
            .copied()
            .ok_or_else(|| CfbError::UnexpectedEndOfChain {
                context: "mini FAT".to_string(),
            })?;
        sectors_read += 1;
    }

    Ok(mini_fat)
}

pub(crate) fn read_stream(
    bytes: &[u8],
    sector_size: usize,
    mini_sector_size: usize,
    mini_stream_cutoff: u32,
    fat: &[u32],
    mini_fat: &[u32],
    mini_stream_start: u32,
    mini_container_size: u64,
    start_sector: u32,
    size: u64,
    context: &str,
) -> Result<Vec<u8>> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let size_usize = usize::try_from(size).map_err(|_| CfbError::StreamTooLarge {
        path: context.to_string(),
        size,
        limit: usize::MAX as u64,
    })?;

    if size as u32 >= mini_stream_cutoff {
        read_regular_stream(bytes, sector_size, fat, start_sector, size_usize, context)
    } else {
        read_mini_stream(
            bytes,
            sector_size,
            mini_sector_size,
            fat,
            mini_fat,
            mini_stream_start,
            mini_container_size,
            start_sector,
            size_usize,
            context,
        )
    }
}

fn read_regular_stream(
    bytes: &[u8],
    sector_size: usize,
    fat: &[u32],
    mut sector_id: u32,
    size: usize,
    context: &str,
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(size);
    let mut steps = 0usize;
    while out.len() < size {
        if steps >= MAX_CHAIN_STEPS {
            return Err(CfbError::ChainTooLong {
                context: context.to_string(),
            });
        }
        if sector_id == END_OF_CHAIN || sector_id == FREE_SECTOR {
            return Err(CfbError::UnexpectedEndOfChain {
                context: context.to_string(),
            });
        }
        let sector = read_sector(bytes, sector_size, sector_id)?;
        let remaining = size - out.len();
        out.extend_from_slice(&sector[..remaining.min(sector.len())]);
        sector_id = fat
            .get(sector_id as usize)
            .copied()
            .ok_or_else(|| CfbError::UnexpectedEndOfChain {
                context: context.to_string(),
            })?;
        steps += 1;
    }
    out.truncate(size);
    Ok(out)
}

fn read_mini_stream(
    bytes: &[u8],
    sector_size: usize,
    mini_sector_size: usize,
    fat: &[u32],
    mini_fat: &[u32],
    mini_stream_start: u32,
    mini_container_size: u64,
    mut mini_sector_id: u32,
    size: usize,
    context: &str,
) -> Result<Vec<u8>> {
    let container_size = usize::try_from(mini_container_size).map_err(|_| CfbError::StreamTooLarge {
        path: "mini stream container".to_string(),
        size: mini_container_size,
        limit: usize::MAX as u64,
    })?;
    let mini_stream = read_regular_stream(
        bytes,
        sector_size,
        fat,
        mini_stream_start,
        container_size,
        "mini stream container",
    )?;
    let mut out = Vec::with_capacity(size);
    let mut steps = 0usize;
    while out.len() < size {
        if steps >= MAX_CHAIN_STEPS {
            return Err(CfbError::ChainTooLong {
                context: context.to_string(),
            });
        }
        if mini_sector_id == END_OF_CHAIN || mini_sector_id == FREE_SECTOR {
            return Err(CfbError::UnexpectedEndOfChain {
                context: context.to_string(),
            });
        }
        let start = mini_sector_id as usize * mini_sector_size;
        let end = start + mini_sector_size;
        if end > mini_stream.len() {
            return Err(CfbError::UnexpectedEndOfChain {
                context: context.to_string(),
            });
        }
        let remaining = size - out.len();
        out.extend_from_slice(&mini_stream[start..start + remaining.min(mini_sector_size)]);
        mini_sector_id = mini_fat
            .get(mini_sector_id as usize)
            .copied()
            .ok_or_else(|| CfbError::UnexpectedEndOfChain {
                context: context.to_string(),
            })?;
        steps += 1;
    }
    out.truncate(size);
    Ok(out)
}

fn read_sector(bytes: &[u8], sector_size: usize, sector_id: u32) -> Result<Vec<u8>> {
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
