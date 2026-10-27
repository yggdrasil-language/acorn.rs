//! `hmtx` — horizontal metrics.
//!
//! The layout is: `numHMetrics` records of (advanceWidth, lsb), then a tail
//! of (numGlyphs - numHMetrics) entries with only lsb (the advance width
//! repeats the last value).

use crate::reader::{ReadError, Reader};

/// A single LongHorMetric entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongHorMetric {
    /// Advance width in font units.
    pub advance_width: u16,
    /// Left side bearing.
    pub lsb: i16,
}

/// Horizontal metrics table.
#[derive(Debug, Clone)]
pub struct HmtxTable {
    /// Per-glyph horizontal metrics — length = `numGlyphs`.
    pub metrics: Vec<LongHorMetric>,
}

impl HmtxTable {
    /// Parse from `hmtx` bytes, knowing `num_glyphs` and `num_hmetrics`.
    pub fn parse(r: &mut Reader<'_>, num_glyphs: u16, num_hmetrics: u16) -> Result<Self, ReadError> {
        let num_glyphs = num_glyphs as usize;
        let num_hmetrics = num_hmetrics as usize;
        let mut metrics = Vec::with_capacity(num_glyphs);

        for _ in 0..num_hmetrics {
            metrics.push(LongHorMetric { advance_width: r.read_u16()?, lsb: r.read_i16()? });
        }

        if let Some(last_adv) = metrics.last().map(|m| m.advance_width) {
            while metrics.len() < num_glyphs {
                metrics.push(LongHorMetric { advance_width: last_adv, lsb: r.read_i16()? });
            }
        }

        Ok(Self { metrics })
    }

    /// Metric for the given glyph ID, if in range.
    #[inline]
    pub fn get(&self, glyph_id: u16) -> Option<LongHorMetric> {
        self.metrics.get(glyph_id as usize).copied()
    }
}
