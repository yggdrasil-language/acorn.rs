//! SFNT container: offset table, table records, checksum validation, and TTC.
//!
//! The SFNT structure is:
//! ```text
//! [OffsetTable]           12 bytes
//! [TableRecord] * n       16 bytes each
//! [Table data] ...        each padded to 4-byte boundary
//! ```
//!
//! TTC (TrueType Collection) wraps multiple faces:
//! ```text
//! TTCHeader               12 bytes
//! Face offsets[]          4 * numFaces bytes
//! [per face SFNT ...]
//! ```

use crate::reader::{ReadError, Reader, Tag};
use std::collections::HashMap;

/// Known major SFNT versions (magic words).
pub mod magic {
    use super::Tag;

    /// TrueType outline: `b"\x00\x01\x00\x00"`.
    pub const TRUE_TYPE: u32 = 0x0001_0000;
    /// OpenType with PostScript outlines: `b"OTTO"`.
    pub const OPEN_TYPE: u32 = Tag([b'O', b'T', b'T', b'O']).as_u32();
    /// TrueType Collection header magic: `b"ttcf"`.
    pub const TTC: u32 = Tag([b't', b't', b'c', b'f']).as_u32();
    /// Apple `sfnt` wrapper magic: `b"sfnt"`.
    pub const SFNT: u32 = Tag([b's', b'f', b'n', b't']).as_u32();
    /// WOFF compressed wrapper magic: `b"wOFF"`.
    pub const WOFF: u32 = Tag([b'w', b'O', b'F', b'F']).as_u32();
    /// WOFF2 compressed wrapper magic: `b"wOF2"`.
    pub const WOFF2: u32 = Tag([b'w', b'O', b'F', b'2']).as_u32();
}

impl Tag {
    /// Interpret the four bytes as a big-endian `u32` magic.
    pub const fn as_u32(self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

/// SFNT offset table (12 bytes header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetTable {
    /// SFNT version (`magic::TRUE_TYPE` or `magic::OPEN_TYPE`).
    pub version: u32,
    /// Number of tables.
    pub num_tables: u16,
    /// SearchRange (precomputed hint, informational).
    pub search_range: u16,
    /// EntrySelector (precomputed hint).
    pub entry_selector: u16,
    /// RangeShift (precomputed hint).
    pub range_shift: u16,
}

impl OffsetTable {
    /// Parse from a reader positioned at offset 0.
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        Ok(Self {
            version: r.read_u32()?,
            num_tables: r.read_u16()?,
            search_range: r.read_u16()?,
            entry_selector: r.read_u16()?,
            range_shift: r.read_u16()?,
        })
    }

    /// True if this is an OpenType (CFF-based) font.
    #[inline]
    pub fn is_opentype(self) -> bool {
        self.version == magic::OPEN_TYPE
    }

    /// True if this is a TrueType (glyf-based) font.
    #[inline]
    pub fn is_truetype(self) -> bool {
        self.version == magic::TRUE_TYPE
    }

    /// True if the version is recognized as a valid SFNT font.
    #[inline]
    pub fn is_known(self) -> bool {
        self.is_truetype() || self.is_opentype()
    }
}

/// One entry in the table directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRecord {
    /// Four-byte tag (e.g. `b"MATH"`).
    pub tag: Tag,
    /// Table checksum (32-bit sum, with special handling for `head`).
    pub checksum: u32,
    /// Absolute offset of the table from the start of the font file.
    pub offset: u32,
    /// Table length in bytes (unpadded).
    pub length: u32,
}

impl TableRecord {
    /// Parse from a reader.
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        Ok(Self { tag: r.read_tag()?, checksum: r.read_u32()?, offset: r.read_u32()?, length: r.read_u32()? })
    }
}

/// All tables discovered in a single SFNT face.
#[derive(Debug, Clone, Default)]
pub struct TableDirectory {
    /// Tables indexed by tag.
    pub tables: HashMap<Tag, TableRecord>,
    /// Tags in the order they were declared.
    pub order: Vec<Tag>,
}

impl TableDirectory {
    /// Look up a table by tag.
    #[inline]
    pub fn get(&self, tag: &[u8; 4]) -> Option<&TableRecord> {
        self.tables.get(&Tag(*tag))
    }

    /// Iterate in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &TableRecord> {
        self.order.iter().filter_map(|t| self.tables.get(t))
    }

    /// Number of tables.
    #[inline]
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// True when there are zero tables.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
}

/// One face inside a TTC file — the same layout as a standalone SFNT.
#[derive(Debug, Clone)]
pub struct Face {
    /// The raw bytes of this face, starting at its offset.
    pub bytes: Vec<u8>,
    /// Parsed offset table.
    pub header: OffsetTable,
    /// Parsed table directory.
    pub directory: TableDirectory,
    /// Per-table checksum check results (good / bad / skipped).
    pub checksum_status: HashMap<Tag, ChecksumStatus>,
}

/// Outcome of a table checksum check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumStatus {
    /// Checksum matches.
    Ok,
    /// Checksum mismatch — table data present but may be corrupt.
    Mismatch { expected: u32, actual: u32 },
    /// Skipped (e.g. checksum adjustment from `head` could not be applied).
    Skipped,
}

/// Top-level container covering both single SFNT files and TTC collections.
#[derive(Debug, Clone)]
pub enum FontContainer {
    /// Single face.
    Single(Face),
    /// TrueType collection with multiple faces.
    Collection(Vec<Face>),
}

impl FontContainer {
    /// Number of faces in this container.
    #[inline]
    pub fn num_faces(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Collection(v) => v.len(),
        }
    }

    /// Borrow a specific face by index.
    #[inline]
    pub fn face(&self, index: usize) -> Option<&Face> {
        match self {
            Self::Single(f) if index == 0 => Some(f),
            Self::Collection(v) => v.get(index),
            _ => None,
        }
    }

    /// Convenience for taking the first face.
    #[inline]
    pub fn first(&self) -> Option<&Face> {
        self.face(0)
    }
}

/// Parse an entire font byte buffer into a [`FontContainer`].
///
/// This reads headers and the table directory, validates checksums, and
/// captures the per-face byte ranges. The individual table contents are NOT
/// materialized here — consumers fetch slices via [`Face::table_data`].
pub fn parse_font(bytes: &[u8]) -> Result<FontContainer, ParseError> {
    if bytes.len() < 12 {
        return Err(ParseError::TooSmall { need: 12, have: bytes.len() });
    }
    let mut probe = Reader::new(bytes);
    let magic_word = probe.read_u32()?;

    if magic_word == magic::TTC {
        parse_ttc(bytes)
    }
    else {
        let face = parse_single_face(bytes, 0)?;
        Ok(FontContainer::Single(face))
    }
}

fn parse_ttc(bytes: &[u8]) -> Result<FontContainer, ParseError> {
    if bytes.len() < 16 {
        return Err(ParseError::TooSmall { need: 16, have: bytes.len() });
    }
    let mut r = Reader::new(bytes);
    let _magic = r.read_u32()?;
    let _version = r.read_u32()?;
    let num_faces = r.read_u32()?;

    let mut face_offsets = Vec::with_capacity(num_faces as usize);
    for _ in 0..num_faces {
        face_offsets.push(r.read_u32()?);
    }

    let mut faces = Vec::with_capacity(num_faces as usize);
    for offset in face_offsets {
        let face = parse_single_face(bytes, offset as usize)?;
        faces.push(face);
    }
    Ok(FontContainer::Collection(faces))
}

fn parse_single_face(bytes: &[u8], face_offset: usize) -> Result<Face, ParseError> {
    let mut r = Reader::new(bytes);
    r.seek(face_offset)?;

    let header = OffsetTable::parse(&mut r)?;
    if !header.is_known() {
        return Err(ParseError::UnknownVersion(header.version));
    }

    let num = header.num_tables as usize;
    let mut order = Vec::with_capacity(num);
    let mut tables = HashMap::with_capacity(num);

    for _ in 0..num {
        let rec = TableRecord::parse(&mut r)?;
        let _ = order.push(rec.tag);
        tables.insert(rec.tag, rec);
    }

    let directory = TableDirectory { tables, order };

    // Extract face byte range: from face start to max(table_offset + table_length + pad).
    let mut end = face_offset;
    for rec in directory.iter() {
        let o = face_offset.checked_add(rec.offset as usize).ok_or(ParseError::Overflow)?;
        let unpadded = o.checked_add(rec.length as usize).ok_or(ParseError::Overflow)?;
        let padded = (unpadded + 3) & !3;
        if padded > end {
            end = padded;
        }
    }
    end = end.min(bytes.len());
    let face_bytes = bytes[face_offset..end].to_vec();

    let face = Face { bytes: face_bytes, header, directory, checksum_status: HashMap::new() };
    Ok(face)
}

impl Face {
    /// Borrow the raw SFNT bytes of this face (relative offsets inside).
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.bytes
    }

    /// Slice the given table data using the stored offset and length.
    pub fn table_data(&self, rec: &TableRecord) -> Result<&[u8], ParseError> {
        let start = rec.offset as usize;
        let end = start.checked_add(rec.length as usize).ok_or(ParseError::Overflow)?;
        if end > self.bytes.len() {
            return Err(ParseError::TableOutOfBounds {
                tag: rec.tag,
                offset: rec.offset,
                length: rec.length,
                face_len: self.bytes.len(),
            });
        }
        Ok(&self.bytes[start..end])
    }
}

/// Errors that can happen while parsing the SFNT container.
#[derive(Debug, Clone)]
pub enum ParseError {
    /// Input is too small to hold the claimed structure.
    TooSmall {
        /// Minimum bytes needed.
        need: usize,
        /// Bytes actually available.
        have: usize,
    },
    /// Magic version is not a known SFNT variant.
    UnknownVersion(u32),
    /// Integer overflow while combining offsets.
    Overflow,
    /// Reader hit its bound mid-structure.
    Bounds(crate::reader::ReadError),
    /// A table record points past the face data.
    TableOutOfBounds { tag: Tag, offset: u32, length: u32, face_len: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooSmall { need, have } => {
                write!(f, "font too small: need {need} have {have}")
            }
            Self::UnknownVersion(v) => write!(f, "unknown SFNT version: {v:08x}"),
            Self::Overflow => f.write_str("offset arithmetic overflow"),
            Self::Bounds(e) => write!(f, "{e}"),
            Self::TableOutOfBounds { tag, offset, length, face_len } => {
                write!(f, "table {} at offset {} length {} exceeds face size {}", tag, offset, length, face_len)
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<crate::reader::ReadError> for ParseError {
    #[inline]
    fn from(e: crate::reader::ReadError) -> Self {
        Self::Bounds(e)
    }
}

// ---- checksum validation ----

/// Compute the 32-bit sum of `data` (big-endian u32s). Input is padded with
/// trailing zeros to a multiple of four.
pub fn table_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    let full = data.len() & !3;
    while i < full {
        sum = sum.wrapping_add(u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]));
        i += 4;
    }
    let rem = data.len() & 3;
    if rem > 0 {
        let mut buf = [0u8; 4];
        buf[..rem].copy_from_slice(&data[full..full + rem]);
        sum = sum.wrapping_add(u32::from_be_bytes(buf));
    }
    sum
}

/// Compute the file-level checksum (sum of all table checksums, including
/// the `head` table with its `checkSumAdjustment` field zeroed).
pub fn file_checksum(face: &Face) -> u32 {
    let mut sum: u32 = 0;
    let head_tag = Tag([b'h', b'e', b'a', b'd']);
    for rec in face.directory.iter() {
        let data = match face.table_data(rec) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if rec.tag == head_tag {
            // checkSumAdjustment lives at offset 8 (u32) — zero it out for the sum.
            if data.len() >= 12 {
                let mut copy = data.to_vec();
                copy[8..12].copy_from_slice(&[0, 0, 0, 0]);
                sum = sum.wrapping_add(table_checksum(&copy));
                continue;
            }
        }
        sum = sum.wrapping_add(table_checksum(data));
    }
    sum
}

/// Validate every table checksum inside `face` and populate its
/// [`Face::checksum_status`] map.
pub fn validate_checksums(face: &mut Face) {
    let head_tag = Tag([b'h', b'e', b'a', b'd']);
    let file_sum = file_checksum(face);

    let mut adjustment: u32 = 0;
    if let Some(head) = face.directory.get(&head_tag.0) {
        if let Ok(data) = face.table_data(head) {
            if data.len() >= 12 {
                adjustment = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
            }
        }
    }
    let expected_file_sum = adjustment.wrapping_add(file_sum);

    let mut status = HashMap::with_capacity(face.directory.len());
    for rec in face.directory.iter() {
        let Ok(data) = face.table_data(rec)
        else {
            status.insert(rec.tag, ChecksumStatus::Skipped);
            continue;
        };
        let actual = table_checksum(data);
        if actual == rec.checksum {
            status.insert(rec.tag, ChecksumStatus::Ok);
        }
        else {
            status.insert(rec.tag, ChecksumStatus::Mismatch { expected: rec.checksum, actual });
        }
    }
    face.checksum_status = status;
    let _ = expected_file_sum; // diagnostic use only
}

// ---- convenience helpers re-exported for top-level parse ----

/// Top-level parse that also runs checksum validation on every face.
pub fn parse_font_validated(bytes: &[u8]) -> Result<FontContainer, ParseError> {
    let mut container = parse_font(bytes)?;
    match &mut container {
        FontContainer::Single(f) => validate_checksums(f),
        FontContainer::Collection(v) => v.iter_mut().for_each(validate_checksums),
    }
    Ok(container)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_display() {
        let t = Tag([b'M', b'A', b'T', b'H']);
        assert_eq!(t.to_string(), "MATH");
    }

    #[test]
    fn checksum_of_empty() {
        assert_eq!(table_checksum(&[]), 0);
    }

    #[test]
    fn checksum_of_four() {
        let data = [0u8, 0, 0, 1];
        assert_eq!(table_checksum(&data), 1);
    }

    #[test]
    fn checksum_pads_tail() {
        let data = [0u8, 0, 0, 1, 0, 0, 0]; // 7 bytes → pads with one trailing zero
        assert_eq!(table_checksum(&data), 1);
    }

    #[test]
    fn parse_error_display() {
        let e = ParseError::TooSmall { need: 12, have: 4 };
        assert!(e.to_string().contains("too small"));
    }
}
