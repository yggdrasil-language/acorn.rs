//! `post` — PostScript information.
//!
//! We only need fields up through format 3 (no glyph names).

use crate::reader::{ReadError, Reader};

/// PostScript table (minimal version 3).
#[derive(Debug, Clone, PartialEq)]
pub struct PostTable {
    /// Table version (1.0 .. 3.0 common).
    pub table_version: f32,
    /// Italic angle (degrees).
    pub italic_angle: f32,
    /// Underline position (relative to baseline).
    pub underline_position: i16,
    /// Underline thickness.
    pub underline_thickness: i16,
    /// Fixed-pitch hint.
    pub is_fixed_pitch: u32,
    /// Minimum memory used while downloading the font (per spec, may be 0).
    pub min_mem_type_42: u32,
    /// Maximum memory used.
    pub max_mem_type_42: u32,
    /// Minimum memory for Type 1 download.
    pub min_mem_type_1: u32,
    /// Maximum memory for Type 1 download.
    pub max_mem_type_1: u32,
}

impl PostTable {
    pub const SIZE: usize = 32;

    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        Ok(Self {
            table_version: r.read_fixed()?,
            italic_angle: r.read_fixed()?,
            underline_position: r.read_i16()?,
            underline_thickness: r.read_i16()?,
            is_fixed_pitch: r.read_u32()?,
            min_mem_type_42: r.read_u32()?,
            max_mem_type_42: r.read_u32()?,
            min_mem_type_1: r.read_u32()?,
            max_mem_type_1: r.read_u32()?,
        })
    }
}
