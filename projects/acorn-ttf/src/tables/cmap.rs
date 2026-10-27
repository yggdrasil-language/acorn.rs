//! `cmap` — character to glyph mapping.
//!
//! Supports subtable formats 0, 4 (segment-to-delta, most common for BMP
//! fonts), and 12 (segmented coverage for SMP). We extract a simple codepoint
//! → glyph mapping for each subtable and also record the raw encoding info.

use crate::reader::{ReadError, Reader};
use std::collections::HashMap;

/// One cmap subtable record inside the header's record table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingRecord {
    /// Platform ID (3 = Windows, 1 = Macintosh, 0 = Unicode).
    pub platform: u16,
    /// Encoding ID within platform (1 = Unicode, 0 = MacRoman, etc.).
    pub encoding: u16,
    /// Byte offset from the start of the `cmap` table to the subtable.
    pub offset: u32,
}

/// Parsed cmap subtable.
#[derive(Debug, Clone)]
pub struct CmapSubtable {
    /// Encoding record context.
    pub platform: u16,
    /// Encoding record context.
    pub encoding: u16,
    /// Subtable format number.
    pub format: u16,
    /// Raw codepoint → glyph ID mapping built from this subtable.
    pub map: HashMap<u32, u16>,
}

/// Top-level cmap table.
#[derive(Debug, Clone)]
pub struct CmapTable {
    /// Table version (0 expected).
    pub version: u16,
    /// Encoding records.
    pub records: Vec<EncodingRecord>,
    /// Parsed subtables.
    pub subtables: Vec<CmapSubtable>,
    /// Deduplicated best-effort mapping (union of all subtables).
    pub unified: HashMap<u32, u16>,
}

impl CmapTable {
    pub fn parse(data: &[u8]) -> Result<Self, ParseCmapError> {
        let mut r = Reader::new(data);
        let version = r.read_u16()?;
        let num_records = r.read_u16()?;

        let mut records = Vec::with_capacity(num_records as usize);
        for _ in 0..num_records {
            records.push(EncodingRecord { platform: r.read_u16()?, encoding: r.read_u16()?, offset: r.read_u32()? });
        }

        let mut subtables = Vec::with_capacity(records.len());
        let mut unified = HashMap::new();

        for rec in &records {
            if rec.offset as usize >= data.len() {
                continue;
            }
            let mut sr = Reader::at(data, rec.offset as usize, data.len() - rec.offset as usize);
            let format = sr.read_u16()?;
            let sub_map = match format {
                0 => parse_format_0(&mut sr)?,
                4 => parse_format_4(&mut sr)?,
                10 => parse_format_10(&mut sr)?,
                12 => parse_format_12(&mut sr)?,
                13 => parse_format_13(&mut sr)?,
                _ => continue, // unknown format → skip
            };
            // Prefer platform 3 encoding 10 (full Unicode) and 3/1 (BMP) first;
            // but just merge everything here. Consumers pick by platform if they care.
            let sub = CmapSubtable { platform: rec.platform, encoding: rec.encoding, format, map: sub_map };
            unified.extend(sub.map.iter().map(|(&k, &v)| (k, v)));
            subtables.push(sub);
        }

        Ok(Self { version, records, subtables, unified })
    }

    /// Look up a Unicode codepoint (full range) → glyph ID.
    #[inline]
    pub fn lookup(&self, cp: u32) -> Option<u16> {
        self.unified.get(&cp).copied()
    }

    /// Iterator over `(codepoint, glyph_id)`.
    pub fn iter(&self) -> impl Iterator<Item = (u32, u16)> + '_ {
        self.unified.iter().map(|(&k, &v)| (k, v))
    }
}

/// Error while parsing `cmap`.
#[derive(Debug, Clone)]
pub enum ParseCmapError {
    /// Bounded reader error.
    Bounds(ReadError),
    /// Malformed cmap subtable (e.g. bad segment count).
    Malformed(String),
}

impl From<ReadError> for ParseCmapError {
    #[inline]
    fn from(e: ReadError) -> Self {
        Self::Bounds(e)
    }
}

impl std::fmt::Display for ParseCmapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bounds(e) => write!(f, "cmap bounds: {e}"),
            Self::Malformed(s) => write!(f, "cmap malformed: {s}"),
        }
    }
}

impl std::error::Error for ParseCmapError {}

fn parse_format_0(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _length = r.read_u16()?;
    let _lang = r.read_u16()?;
    let mut map = HashMap::new();
    for cp in 0u32..256 {
        let g = r.read_u8()?;
        if g != 0 {
            map.insert(cp, g as u16);
        }
    }
    Ok(map)
}

fn parse_format_4(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _length = r.read_u16()?;
    let _lang = r.read_u16()?;
    let seg_count_x2 = r.read_u16()?;
    if seg_count_x2 % 2 != 0 {
        return Err(ParseCmapError::Malformed("odd segCountX2".into()));
    }
    let seg_count = (seg_count_x2 / 2) as usize;
    let _search_range = r.read_u16()?;
    let _entry_selector = r.read_u16()?;
    let _range_shift = r.read_u16()?;

    let mut end_codes = Vec::with_capacity(seg_count);
    for _ in 0..seg_count {
        end_codes.push(r.read_u16()?);
    }
    let _pad = r.read_u16()?; // reservedPad
    let mut start_codes = Vec::with_capacity(seg_count);
    for _ in 0..seg_count {
        start_codes.push(r.read_u16()?);
    }
    let mut deltas = Vec::with_capacity(seg_count);
    for _ in 0..seg_count {
        deltas.push(r.read_i16()?);
    }
    // The `idRangeOffset` array starts here. Each entry is a *self-relative*
    // byte offset (OpenType §4.3), so the entry's own address has to be added
    // back before indexing `glyphIdArray`.
    let id_range_offsets_start = r.position();
    let mut ranges = Vec::with_capacity(seg_count);
    for _ in 0..seg_count {
        ranges.push(r.read_u16()?);
    }
    let limit = r.limit;

    let mut map = HashMap::new();
    for i in 0..seg_count {
        let start = start_codes[i] as u32;
        let end = end_codes[i] as u32;
        if end == 0xFFFF && start == 0xFFFF {
            break; // terminal sentinel
        }
        let delta = deltas[i];
        let range_off = ranges[i] as usize;

        for cp in start..=end {
            let glyph = if range_off == 0 {
                let g = (cp as i32 + delta as i32) as u16;
                if g != 0 { Some(g) } else { None }
            }
            else {
                // address_of_idRangeOffset[i] + idRangeOffset[i] + 2 * (cp - start)
                let address = id_range_offsets_start + i * 2 + range_off + ((cp - start) as usize * 2);
                if address + 2 > limit {
                    break;
                }
                let g = u16::from_be_bytes([r.bytes[address], r.bytes[address + 1]]);
                if g == 0 {
                    None
                }
                else {
                    let adjusted = (g as i32 + delta as i32) as u16;
                    if adjusted == 0 { None } else { Some(adjusted) }
                }
            };
            if let Some(g) = glyph {
                map.insert(cp, g);
            }
        }
    }
    Ok(map)
}

fn parse_format_10(_r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    // Format 10 is a dense 32-bit packed map — rare. Stub.
    Ok(HashMap::new())
}

fn parse_format_12(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _length = r.read_u32()?;
    let _lang = r.read_u32()?;
    let num_groups = r.read_u32()? as usize;
    let mut map = HashMap::new();
    for _ in 0..num_groups {
        let start_cp = r.read_u32()?;
        let end_cp = r.read_u32()?;
        let start_glyph = r.read_u32()? as u16;
        let mut g = start_glyph;
        for cp in start_cp..=end_cp {
            map.insert(cp, g);
            g = g.wrapping_add(1);
        }
    }
    Ok(map)
}

fn parse_format_13(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _length = r.read_u32()?;
    let _lang = r.read_u32()?;
    let num_groups = r.read_u32()? as usize;
    let mut map = HashMap::new();
    for _ in 0..num_groups {
        let start_cp = r.read_u32()?;
        let end_cp = r.read_u32()?;
        let g = r.read_u32()? as u16;
        for cp in start_cp..=end_cp {
            map.insert(cp, g);
        }
    }
    Ok(map)
}
