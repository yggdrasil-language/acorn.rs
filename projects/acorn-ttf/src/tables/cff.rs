//! CFF (PostScript Compact Font Format) header detection — minimal stub.
//!
//! We intentionally stop at the Index level. Full CFF parsing is a 2000+
//! LOC project that does not belong in the vertical slice: VuTeX currently
//! uses ttf-parser which ships a CFF rasterizer, and we only need to
//! recognise CFF/CFF2 so that a [`crate::sfnt::OffsetTable`] marked as
//! OpenType (`b"OTTO"`) can be annotated correctly.

use crate::reader::{ReadError, Reader};

/// CFF header (the first table under CFF's OffsetTable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CffInfo {
    /// Major version (1 for CFF Type1, 2 for CFF2).
    pub major: u8,
    /// Minor version (usually 0).
    pub minor: u8,
    /// OffsetSize (byte width of offset pointers).
    pub offset_size: u8,
    /// OffSize of the name INDEX.
    pub name_index_size: u8,
    /// Name INDEX length.
    pub name_index_count: u32,
}

/// A CFF INDEX table — array of variable-length objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CffIndex {
    /// OffSize (1..4).
    pub offset_size: u8,
    /// Number of entries.
    pub count: u32,
    /// Start offsets (count + 1 entries).
    pub offsets: Vec<u32>,
}

/// Try to read the CFF table header. Returns `None` if the blob is too small
/// or not CFF.
pub fn parse_cff_header(data: &[u8]) -> Option<CffInfo> {
    if data.len() < 6 {
        return None;
    }
    let mut r = Reader::new(data);
    let major = r.read_u8().ok()?;
    let minor = r.read_u8().ok()?;
    let offset_size = r.read_u8().ok()?;
    let _ = r.read_u8(); // reserved

    if major != 1 {
        return None;
    }
    if offset_size < 1 || offset_size > 4 {
        return None;
    }

    // Name INDEX immediately follows the header.
    let name_index_size = offset_size;
    let name_index_count = read_index_count(&mut r).ok()?;
    Some(CffInfo { major, minor, offset_size, name_index_size, name_index_count })
}

fn read_index_count(r: &mut Reader<'_>) -> Result<u32, ReadError> {
    let count = r.read_u16()? as u32;
    let off_size = r.read_u8()?;
    // Skip offsets.
    r.skip((count + 1) as usize * off_size as usize)?;
    Ok(count)
}
