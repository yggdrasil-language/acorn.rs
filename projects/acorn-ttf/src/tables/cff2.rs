//! CFF2 detection — pure stub.
//!
//! CFF2 (major version 2) is the newer, non-backward-compatible CFF variant
//! used by variable OTF fonts. Our target LatinModernMath is CFF1.

use crate::reader::{ReadError, Reader};

/// CFF2 header info (very small — nothing beyond versioning is needed for detection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cff2Info {
    pub major: u8,
    pub minor: u8,
}

/// Parse the 4-byte CFF2 header; returns `None` if the blob is not CFF2.
pub fn parse_cff2_header(data: &[u8]) -> Option<Cff2Info> {
    if data.len() < 4 {
        return None;
    }
    let mut r = Reader::new(data);
    let major = r.read_u8().ok()?;
    let minor = r.read_u8().ok()?;
    let _offset_size = r.read_u8().ok()?;
    let _ = r.read_u8(); // reserved
    if major != 2 {
        return None;
    }
    Some(Cff2Info { major, minor })
}

#[allow(dead_code)]
fn _parse_index_count(r: &mut Reader<'_>) -> Result<u32, ReadError> {
    let count = r.read_u32()?;
    let off_size = r.read_u8()?;
    r.skip((count + 1) as usize * off_size as usize)?;
    Ok(count)
}
