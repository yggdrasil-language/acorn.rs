//! `cmap` — character to glyph mapping.
//!
//! Supports subtable formats 0, 4 (segment-to-delta, the most common BMP
//! layout), 6 (trimmed table), 10 (trimmed array), 12 (segmented coverage) and
//! 13 (many-to-one range). Formats this crate does not implement (2, 8, 14 and
//! anything unknown) are kept as [`CmapSubtableData::Unsupported`], so callers
//! can tell "this font does not map the character" apart from "this crate did
//! not read that subtable".
//!
//! [`CmapTable::unified`] merges Unicode subtables under a fixed precedence
//! (see [`CmapTable::parse`]). Macintosh subtables are parsed and exposed but
//! never contribute to the unified map, because their byte codes are not
//! Unicode scalar values.

use crate::reader::{ReadError, Reader};
use std::collections::HashMap;

/// Highest Unicode scalar value. Format 12/13 ranges beyond it are ignored.
const MAX_UNICODE: u32 = 0x10_FFFF;

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

/// Payload of one parsed cmap subtable.
#[derive(Debug, Clone)]
pub enum CmapSubtableData {
    /// Codepoint → glyph ID mapping decoded from a supported format.
    Map(HashMap<u32, u16>),
    /// The encoding record points at a format this crate does not implement.
    ///
    /// The record is preserved so consumers can diagnose it instead of
    /// mistaking an unread subtable for an empty one.
    Unsupported,
}

impl CmapSubtableData {
    /// Borrow the decoded map, or `None` when the format is unsupported.
    #[inline]
    pub fn map(&self) -> Option<&HashMap<u32, u16>> {
        match self {
            Self::Map(map) => Some(map),
            Self::Unsupported => None,
        }
    }
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
    /// Decoded mapping or an explicit unsupported marker.
    pub data: CmapSubtableData,
}

impl CmapSubtable {
    /// Borrow this subtable's codepoint → glyph map, if its format is supported.
    #[inline]
    pub fn map(&self) -> Option<&HashMap<u32, u16>> {
        self.data.map()
    }
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
    /// Best-effort Unicode mapping merged from the Unicode subtables.
    pub unified: HashMap<u32, u16>,
}

impl CmapTable {
    /// Parse a `cmap` table payload.
    ///
    /// `unified` is built by merging every Unicode subtable (platform 0 and
    /// platform 3) in ascending precedence, so the most authoritative record
    /// wins each duplicate codepoint:
    ///
    /// 1. Windows UCS-4 (`3/10`)
    /// 2. Windows Unicode BMP (`3/1`)
    /// 3. Unicode full repertoire (`0/4`, `0/6`)
    /// 4. other Unicode platform encodings
    /// 5. Windows Symbol (`3/0`)
    ///
    /// Macintosh subtables (platform 1) are skipped for the unified map: their
    /// byte codes are Mac Roman or Mac CJK, not Unicode scalar values, so
    /// merging them would answer lookups for codepoints the font never claims.
    pub fn parse(data: &[u8]) -> Result<Self, ParseCmapError> {
        let mut r = Reader::new(data);
        let version = r.read_u16()?;
        let num_records = r.read_u16()?;

        let mut records = Vec::with_capacity(num_records as usize);
        for _ in 0..num_records {
            records.push(EncodingRecord { platform: r.read_u16()?, encoding: r.read_u16()?, offset: r.read_u32()? });
        }

        let mut subtables = Vec::with_capacity(records.len());
        for rec in &records {
            let offset = rec.offset as usize;
            if offset >= data.len() {
                continue;
            }
            let mut sr = Reader::at(data, offset, data.len() - offset);
            let format = sr.read_u16()?;
            let body = match format {
                0 => CmapSubtableData::Map(parse_format_0(&mut sr)?),
                4 => CmapSubtableData::Map(parse_format_4(&mut sr)?),
                6 => CmapSubtableData::Map(parse_format_6(&mut sr)?),
                10 => CmapSubtableData::Map(parse_format_10(&mut sr)?),
                12 => CmapSubtableData::Map(parse_format_12(&mut sr)?),
                13 => CmapSubtableData::Map(parse_format_13(&mut sr)?),
                _ => CmapSubtableData::Unsupported,
            };
            subtables.push(CmapSubtable { platform: rec.platform, encoding: rec.encoding, format, data: body });
        }

        let mut unified = HashMap::new();
        let mut ranked: Vec<(u8, &CmapSubtable)> = subtables
            .iter()
            .filter_map(|sub| unicode_rank(sub.platform, sub.encoding).map(|rank| (rank, sub)))
            .collect();
        // Ascending precedence: the authoritative subtable is merged last and
        // therefore wins every duplicate codepoint.
        ranked.sort_by_key(|(rank, _)| *rank);
        for (_, sub) in ranked {
            if let Some(map) = sub.map() {
                unified.extend(map.iter().map(|(&cp, &gid)| (cp, gid)));
            }
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

/// Precedence of a subtable for the unified map. Lower ranks win.
///
/// `None` means the record must not feed `unified` (Macintosh and unknown
/// platforms).
fn unicode_rank(platform: u16, encoding: u16) -> Option<u8> {
    match (platform, encoding) {
        (3, 10) => Some(0),
        (3, 1) => Some(1),
        (0, 4) | (0, 6) => Some(2),
        (0, _) => Some(3),
        (3, 0) => Some(4),
        _ => None,
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
    if seg_count_x2 == 0 || seg_count_x2 % 2 != 0 {
        return Err(ParseCmapError::Malformed("odd or zero segCountX2".into()));
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
        if start == 0xFFFF && end == 0xFFFF {
            break; // terminal sentinel
        }
        if start > end {
            continue;
        }
        let delta = deltas[i];
        let range_off = ranges[i] as usize;
        if range_off == 0xFFFF {
            // Some malformed fonts pad the last segment with 0xFFFF, which can
            // never address a glyph array.
            continue;
        }

        for cp in start..=end {
            let glyph = if range_off == 0 {
                match (cp as i32).wrapping_add(delta as i32) as u16 {
                    0 => None,
                    g => Some(g),
                }
            }
            else {
                // address_of_idRangeOffset[i] + idRangeOffset[i] + 2 * (cp - start)
                let address = id_range_offsets_start + i * 2 + range_off + ((cp - start) as usize * 2);
                if address + 2 > limit {
                    break;
                }
                let raw = u16::from_be_bytes([r.bytes[address], r.bytes[address + 1]]);
                if raw == 0 {
                    None // glyphIdArray 0 means "no mapping"
                }
                else {
                    match (raw as i16).wrapping_add(delta) as u16 {
                        0 => None,
                        g => Some(g),
                    }
                }
            };
            if let Some(g) = glyph {
                map.insert(cp, g);
            }
        }
    }
    Ok(map)
}

fn parse_format_6(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _length = r.read_u16()?;
    let _lang = r.read_u16()?;
    let first = r.read_u16()? as u32;
    let count = r.read_u16()? as u32;
    let mut map = HashMap::new();
    for i in 0..count {
        let g = r.read_u16()?;
        if g != 0 {
            map.insert(first + i, g);
        }
    }
    Ok(map)
}

fn parse_format_10(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _reserved = r.read_u16()?;
    let _length = r.read_u32()?;
    let _lang = r.read_u32()?;
    let first = r.read_u32()?;
    let count = r.read_u32()?;
    let mut map = HashMap::new();
    for i in 0..count {
        let Some(cp) = first.checked_add(i).filter(|cp| *cp <= MAX_UNICODE)
        else {
            break;
        };
        let g = r.read_u16()?;
        if g != 0 {
            map.insert(cp, g);
        }
    }
    Ok(map)
}

fn parse_format_12(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _reserved = r.read_u16()?;
    let _length = r.read_u32()?;
    let _lang = r.read_u32()?;
    let num_groups = r.read_u32()?;
    let mut map = HashMap::new();
    for group in 0..num_groups {
        let start_cp = r.read_u32()?;
        let end_cp = r.read_u32()?;
        let start_glyph = r.read_u32()?;
        if start_cp > end_cp {
            return Err(ParseCmapError::Malformed(format!(
                "format 12 group {group} starts at {start_cp:#X} above its end {end_cp:#X}"
            )));
        }
        let end = end_cp.min(MAX_UNICODE);
        for cp in start_cp..=end {
            let gid = start_glyph + (cp - start_cp);
            let gid = u16::try_from(gid).map_err(|_| {
                ParseCmapError::Malformed(format!("format 12 group {group} glyph id {gid} exceeds u16"))
            })?;
            if gid != 0 {
                map.insert(cp, gid);
            }
        }
    }
    Ok(map)
}

fn parse_format_13(r: &mut Reader<'_>) -> Result<HashMap<u32, u16>, ParseCmapError> {
    let _reserved = r.read_u16()?;
    let _length = r.read_u32()?;
    let _lang = r.read_u32()?;
    let num_groups = r.read_u32()?;
    let mut map = HashMap::new();
    for group in 0..num_groups {
        let start_cp = r.read_u32()?;
        let end_cp = r.read_u32()?;
        let raw_gid = r.read_u32()?;
        if start_cp > end_cp {
            return Err(ParseCmapError::Malformed(format!(
                "format 13 group {group} starts at {start_cp:#X} above its end {end_cp:#X}"
            )));
        }
        let gid = u16::try_from(raw_gid).map_err(|_| {
            ParseCmapError::Malformed(format!("format 13 group {group} glyph id {raw_gid} exceeds u16"))
        })?;
        if gid == 0 {
            continue;
        }
        let end = end_cp.min(MAX_UNICODE);
        for cp in start_cp..=end {
            map.insert(cp, gid);
        }
    }
    Ok(map)
}
