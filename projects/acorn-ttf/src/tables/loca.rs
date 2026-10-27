//! `loca` — glyph data offsets.
//!
//! The loca table stores `numGlyphs + 1` offsets into `glyf`. Offsets are
//! either u16 values (half-bytes, format 0) or u32 values (bytes, format 1).
//! The size of glyph `i` is `offsets[i+1] - offsets[i]` in bytes.

use crate::reader::{ReadError, Reader};

/// Loc storage scheme — set by `head.indexToLocFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaFormat {
    /// Offsets are u16 half-bytes.
    Short,
    /// Offsets are u32 bytes.
    Long,
}

impl LocaFormat {
    pub fn from_head(value: i16) -> Self {
        if value == 1 { Self::Long } else { Self::Short }
    }
}

/// Parsed loca: byte offsets, length `numGlyphs + 1`.
#[derive(Debug, Clone)]
pub struct Loca {
    /// Offset array in bytes (format 0 expanded; format 1 raw).
    pub offsets: Vec<u32>,
}

impl Loca {
    /// Size of glyph `glyph_id` in bytes; 0 if the glyph is missing or empty.
    #[inline]
    pub fn glyph_size(&self, glyph_id: u16) -> u32 {
        let idx = glyph_id as usize;
        if idx + 1 >= self.offsets.len() {
            return 0;
        }
        self.offsets[idx + 1].saturating_sub(self.offsets[idx])
    }

    /// Absolute offset (into glyf) for `glyph_id`.
    #[inline]
    pub fn glyph_offset(&self, glyph_id: u16) -> u32 {
        let idx = glyph_id as usize;
        self.offsets.get(idx).copied().unwrap_or(0)
    }
}

/// Parse a `loca` table given the format and `numGlyphs`.
pub fn parse_loca(r: &mut Reader<'_>, num_glyphs: u16, format: LocaFormat) -> Result<Loca, ReadError> {
    let count = num_glyphs as usize + 1;
    let mut offsets = Vec::with_capacity(count);
    match format {
        LocaFormat::Short => {
            for _ in 0..count {
                let half = r.read_u16()? as u32;
                offsets.push(half * 2);
            }
        }
        LocaFormat::Long => {
            for _ in 0..count {
                offsets.push(r.read_u32()?);
            }
        }
    }
    Ok(Loca { offsets })
}
