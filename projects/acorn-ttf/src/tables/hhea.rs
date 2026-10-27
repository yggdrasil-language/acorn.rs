//! `hhea` — horizontal header.

use crate::reader::{ReadError, Reader};

/// Horizontal header.
#[derive(Debug, Clone, PartialEq)]
pub struct HheaTable {
    /// Table version (1.0 expected).
    pub table_version: f32,
    /// Font ascent (in font units).
    pub ascent: i16,
    /// Font descent (in font units, typically negative).
    pub descent: i16,
    /// Font line gap.
    pub line_gap: i16,
    /// Maximum advance width among all glyphs.
    pub advance_width_max: u16,
    /// LSB min.
    pub min_left_side_bearing: i16,
    /// RSB min.
    pub min_right_side_bearing: i16,
    /// Max x extent (from LSB).
    pub x_max_extent: i16,
    /// Caret slope rise (for slanted fonts).
    pub caret_slope_rise: i16,
    /// Caret slope run.
    pub caret_slope_run: i16,
    /// Caret offset.
    pub caret_offset: i16,
    /// 4 reserved i16s.
    pub reserved: [i16; 4],
    /// 0 = current table format.
    pub metric_data_format: i16,
    /// Number of LongHorMetric records in `hmtx`.
    pub num_hmetrics: u16,
}

impl HheaTable {
    pub const SIZE: usize = 36;

    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        Ok(Self {
            table_version: r.read_fixed()?,
            ascent: r.read_i16()?,
            descent: r.read_i16()?,
            line_gap: r.read_i16()?,
            advance_width_max: r.read_u16()?,
            min_left_side_bearing: r.read_i16()?,
            min_right_side_bearing: r.read_i16()?,
            x_max_extent: r.read_i16()?,
            caret_slope_rise: r.read_i16()?,
            caret_slope_run: r.read_i16()?,
            caret_offset: r.read_i16()?,
            reserved: [r.read_i16()?, r.read_i16()?, r.read_i16()?, r.read_i16()?],
            metric_data_format: r.read_i16()?,
            num_hmetrics: r.read_u16()?,
        })
    }
}
