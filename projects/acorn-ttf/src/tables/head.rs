//! `head` — SFNT font header.

use crate::reader::{ReadError, Reader};

/// Font header (`head` table).
#[derive(Debug, Clone, PartialEq)]
pub struct HeadTable {
    /// Table version (1.0 expected).
    pub table_version: f32,
    /// Font revision set by designer.
    pub font_revision: f32,
    /// Checksum adjustment field (zeros `head` contribution to file sum).
    pub checksum_adjustment: u32,
    /// Magic `0x5F0F3CF5`.
    pub magic_number: u32,
    /// Header flags.
    pub flags: u16,
    /// Units per EM (typographic grid).
    pub units_per_em: u16,
    /// Creation date since 1904-01-01 in SFNT epoch.
    pub created: i64,
    /// Modification date since 1904-01-01 in SFNT epoch.
    pub modified: i64,
    /// Bounding box.
    pub x_min: i16,
    /// Bounding box.
    pub y_min: i16,
    /// Bounding box.
    pub x_max: i16,
    /// Bounding box.
    pub y_max: i16,
    /// Bold / italic / regular style bitmask.
    pub mac_style: u16,
    /// Smallest readable pixel size.
    pub lowest_rec_ppem: u16,
    /// `head` table direction hint.
    pub font_direction_hint: i16,
    /// `loca` storage scheme — short (0) or long (1).
    pub index_to_loc_format: i16,
    /// Glyph data format (0 for current format).
    pub glyph_data_format: i16,
}

/// Bits for [`HeadTable::mac_style`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacStyle(pub u16);

impl MacStyle {
    pub const BOLD: u16 = 1 << 0;
    pub const ITALIC: u16 = 1 << 1;
    pub const UNDERLINE: u16 = 1 << 2;
    pub const OUTLINE: u16 = 1 << 3;
    pub const SHADOW: u16 = 1 << 4;
    pub const CONDENSED: u16 = 1 << 5;
    pub const EXTENDED: u16 = 1 << 6;

    #[inline]
    pub fn bold(self) -> bool {
        self.0 & Self::BOLD != 0
    }
    #[inline]
    pub fn italic(self) -> bool {
        self.0 & Self::ITALIC != 0
    }
}

impl HeadTable {
    /// Minimum size (54 bytes).
    pub const SIZE: usize = 54;

    /// Parse from a reader restricted to the `head` table payload.
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        Ok(Self {
            table_version: r.read_fixed()?,
            font_revision: r.read_fixed()?,
            checksum_adjustment: r.read_u32()?,
            magic_number: r.read_u32()?,
            flags: r.read_u16()?,
            units_per_em: r.read_u16()?,
            created: r.read_i64()?,
            modified: r.read_i64()?,
            x_min: r.read_i16()?,
            y_min: r.read_i16()?,
            x_max: r.read_i16()?,
            y_max: r.read_i16()?,
            mac_style: r.read_u16()?,
            lowest_rec_ppem: r.read_u16()?,
            font_direction_hint: r.read_i16()?,
            index_to_loc_format: r.read_i16()?,
            glyph_data_format: r.read_i16()?,
        })
    }

    /// True when the header `indexToLocFormat` requests the long (4-byte) loca format.
    #[inline]
    pub fn uses_long_loca(&self) -> bool {
        self.index_to_loc_format == 1
    }
}
