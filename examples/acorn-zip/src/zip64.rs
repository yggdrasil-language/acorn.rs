use crate::eocd::{read_u16_le, read_u32_le, read_u64_le};

pub const ZIP64_EXTRA_ID: u16 = 0x0001;

const ZIP64_EOCD_LOCATOR_SIG: u32 = 0x0706_4b50;
const ZIP64_EOCD_SIG: u32 = 0x0606_4b50;

/// Resolved sizes and offsets that may come from ZIP64 extra fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zip64Fields {
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub local_header_offset: u64,
}

/// Reads ZIP64 extra block `0x0001` after the fixed CD or local header fields.
pub fn parse_zip64_extra(
    bytes: &[u8],
    extra_start: usize,
    extra_length: usize,
    needs_uncompressed: bool,
    needs_compressed: bool,
    needs_local_offset: bool,
    fallback_uncompressed: u32,
    fallback_compressed: u32,
    fallback_local_offset: u32,
) -> Option<Zip64Fields> {
    let extra_end = extra_start.checked_add(extra_length)?;
    if extra_end > bytes.len() {
        return None;
    }
    let mut cursor = extra_start;
    let mut uncompressed = None;
    let mut compressed = None;
    let mut local_offset = None;

    while cursor + 4 <= extra_end {
        let header_id = read_u16_le(bytes, cursor)?;
        let data_size = read_u16_le(bytes, cursor + 2)? as usize;
        cursor += 4;
        let data_end = cursor.checked_add(data_size)?;
        if data_end > extra_end {
            return None;
        }
        if header_id == ZIP64_EXTRA_ID {
            let mut data_cursor = cursor;
            if needs_uncompressed {
                uncompressed = Some(read_u64_le(bytes, data_cursor)?);
                data_cursor += 8;
            }
            if needs_compressed {
                compressed = Some(read_u64_le(bytes, data_cursor)?);
                data_cursor += 8;
            }
            if needs_local_offset {
                local_offset = Some(read_u64_le(bytes, data_cursor)?);
            }
        }
        cursor = data_end;
    }

    Some(Zip64Fields {
        uncompressed_size: uncompressed.unwrap_or(fallback_uncompressed as u64),
        compressed_size: compressed.unwrap_or(fallback_compressed as u64),
        local_header_offset: local_offset.unwrap_or(fallback_local_offset as u64),
    })
}

/// Locates 64-bit EOCD fields when the classic EOCD uses sentinel values.
pub fn resolve_zip64_eocd(
    bytes: &[u8],
    eocd_offset: usize,
) -> Option<(u64, u64, u64)> {
    if eocd_offset < 20 {
        return None;
    }
    let locator_offset = eocd_offset - 20;
    if read_u32_le(bytes, locator_offset)? != ZIP64_EOCD_LOCATOR_SIG {
        return None;
    }
    let zip64_eocd_offset = read_u64_le(bytes, locator_offset + 8)? as usize;
    if read_u32_le(bytes, zip64_eocd_offset)? != ZIP64_EOCD_SIG {
        return None;
    }
    let entry_count = read_u64_le(bytes, zip64_eocd_offset + 32)?;
    let cd_size = read_u64_le(bytes, zip64_eocd_offset + 40)?;
    let cd_offset = read_u64_le(bytes, zip64_eocd_offset + 48)?;
    Some((entry_count, cd_size, cd_offset))
}

pub(crate) fn needs_zip64_eocd(entry_count: u16, cd_size: u32, cd_offset: u32) -> bool {
    entry_count == u16::MAX
        || cd_size == u32::MAX
        || cd_offset == u32::MAX
}

pub(crate) fn needs_zip64_size(value: u32) -> bool {
    value == u32::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zip64_extra_uncompressed_size() {
        let uncompressed = 0x1_0000_0200u64;
        let mut extra = Vec::new();
        extra.extend_from_slice(&ZIP64_EXTRA_ID.to_le_bytes());
        extra.extend_from_slice(&8u16.to_le_bytes());
        extra.extend_from_slice(&uncompressed.to_le_bytes());

        let fields = parse_zip64_extra(
            &extra,
            0,
            extra.len(),
            true,
            false,
            false,
            0,
            512,
            0,
        )
            .expect("zip64 extra");
        assert_eq!(fields.uncompressed_size, uncompressed);
        assert_eq!(fields.compressed_size, 512);
        assert_eq!(fields.local_header_offset, 0);
    }
}
