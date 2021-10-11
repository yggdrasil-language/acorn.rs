//! `maxp` — maximum profile.
//!
//! Two versions are common: 0.5 (old-style, only `numGlyphs`) and 1.0 (all
//! TrueType/OpenType metrics). The parser handles both.

use crate::reader::{ReadError, Reader};

/// Maximum profile.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MaxpTable {
    /// Table version (0.5 or 1.0).
    pub version: f32,
    /// Total glyph count in the font.
    pub num_glyphs: u16,

    // --- Optional (version 1.0 only) ---
    /// Max points in a simple glyph.
    pub max_points: Option<u16>,
    /// Max contours in a simple glyph.
    pub max_contours: Option<u16>,
    /// Max composite glyph points.
    pub max_composite_points: Option<u16>,
    /// Max composite contours.
    pub max_composite_contours: Option<u16>,
    /// Max element references in a compound glyph.
    pub max_zones: Option<u16>,
    /// Max points used by composite glyph zones.
    pub max_twilight_points: Option<u16>,
    /// Max storage area size.
    pub max_storage: Option<u16>,
    /// Max function definitions.
    pub max_function_defs: Option<u16>,
    /// Max instruction definitions.
    pub max_instruction_defs: Option<u16>,
    /// Max stack depth.
    pub max_stack_elements: Option<u16>,
    /// Max size of instructions per glyph.
    pub max_size_of_instructions: Option<u16>,
    /// Max element components in a composite glyph.
    pub max_component_elements: Option<u16>,
    /// Max element depth of composite glyphs.
    pub max_component_depth: Option<u16>,
}

impl MaxpTable {
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        let version = r.read_fixed()?;
        let num_glyphs = r.read_u16()?;

        // Version 0.5 (old-style CFF-only) has EXACTLY 6 bytes total:
        // 4 for the version word + 2 for numGlyphs. Anything beyond that is
        // version 1.0.
        let is_v05 = version.abs() < 1e-6 || (version - 0.5).abs() < 1e-6 || r.remaining() == 0;

        if is_v05 {
            return Ok(Self {
                version,
                num_glyphs,
                max_points: None,
                max_contours: None,
                max_composite_points: None,
                max_composite_contours: None,
                max_zones: None,
                max_twilight_points: None,
                max_storage: None,
                max_function_defs: None,
                max_instruction_defs: None,
                max_stack_elements: None,
                max_size_of_instructions: None,
                max_component_elements: None,
                max_component_depth: None,
            });
        }

        // Version 1.0
        Ok(Self {
            version,
            num_glyphs,
            max_points: Some(r.read_u16()?),
            max_contours: Some(r.read_u16()?),
            max_composite_points: Some(r.read_u16()?),
            max_composite_contours: Some(r.read_u16()?),
            max_zones: Some(r.read_u16()?),
            max_twilight_points: Some(r.read_u16()?),
            max_storage: Some(r.read_u16()?),
            max_function_defs: Some(r.read_u16()?),
            max_instruction_defs: Some(r.read_u16()?),
            max_stack_elements: Some(r.read_u16()?),
            max_size_of_instructions: Some(r.read_u16()?),
            max_component_elements: Some(r.read_u16()?),
            max_component_depth: Some(r.read_u16()?),
        })
    }
}
