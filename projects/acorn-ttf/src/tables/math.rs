//! `MATH` — mathematical layout table.
//!
//! Spec: [MS-OTTO §3.1](https://learn.microsoft.com/en-us/typography/opentype/spec/math).
//!
//! This is the core deliverable for VuTeX formula layout. We parse:
//! 1. `MathConstants` — the 56 numeric settings controlling super/subscript
//!    placement, fractions, bars and radicals.
//! 2. `MathGlyphInfo` — italic correction, accent attachment and math kern
//!    records for every glyph that participates in math shaping.
//! 3. `MathVariants` — the horizontal and vertical constructions per delimiter
//!    glyph, each with its `GlyphAssembly` recipe where the font provides one.

use crate::reader::{ReadError, Reader};
use std::collections::HashMap;

// ---- shared helpers ----

/// Hard cap on the glyph IDs expanded from one Coverage table.
const MAX_COVERAGE_GLYPHS: usize = 1 << 16;
/// Hard cap on variant records inside one `MathGlyphConstruction`.
const MAX_VARIANTS_PER_CONSTRUCTION: usize = 4096;
/// Hard cap on parts inside one `GlyphAssembly`.
const MAX_ASSEMBLY_PARTS: usize = 1024;
/// Hard cap on correction heights inside one `MathKern` subtable.
const MAX_KERN_HEIGHTS: usize = 1024;

/// Build a reader over `start..` of `data`.
///
/// An offset past the end of the table payload is a malformed table, not a
/// truncation: the table directory already told us how long the table is.
fn window(data: &[u8], start: usize) -> Result<Reader<'_>, ParseMathError> {
    if start > data.len() {
        return Err(ParseMathError::Malformed(format!(
            "subtable offset {start} is past the table length {}",
            data.len()
        )));
    }
    Ok(Reader::at(data, start, data.len() - start))
}

/// Resolve an Offset16 that is relative to `base`.
///
/// A zero offset means "this subtable is absent" and yields `None`. Any other
/// offset has to land inside `data`.
fn subtable<'a>(
    data: &'a [u8],
    base: usize,
    offset: usize,
    what: &'static str,
) -> Result<Option<Reader<'a>>, ParseMathError> {
    if offset == 0 {
        return Ok(None);
    }
    let abs = base
        .checked_add(offset)
        .ok_or_else(|| ParseMathError::Malformed(format!("{what} offset {offset} overflows")))?;
    if abs > data.len() {
        return Err(ParseMathError::Malformed(format!(
            "{what} offset {offset} resolves to {abs}, past the table length {}",
            data.len()
        )));
    }
    Ok(Some(Reader::at(data, abs, data.len() - abs)))
}

/// Read one MathValueRecord and keep the value in font units.
///
/// The trailing `uint16` is a Device-table offset used for hinting. It is
/// validated by the bounded read and then discarded.
fn read_math_value(r: &mut Reader<'_>) -> Result<f32, ReadError> {
    let value = r.read_fword()?;
    let _device_table_offset = r.read_u16()?;
    Ok(value)
}

/// Reject glyph IDs that cannot exist in a font with `num_glyphs` glyphs.
fn check_glyph(glyph_id: u16, num_glyphs: Option<u16>, what: &'static str) -> Result<(), ParseMathError> {
    match num_glyphs {
        Some(num_glyphs) if glyph_id >= num_glyphs => {
            Err(ParseMathError::GlyphIdOutOfRange { glyph_id, num_glyphs, what })
        }
        _ => Ok(()),
    }
}

// ---- Coverage tables ----

/// Parse the Coverage table at `base + offset` into an ordered glyph list.
///
/// Format 1 stores an explicit glyph array, format 2 stores ranges. Ranges are
/// expanded with a hard cap so a pathological `start..=end` pair cannot make
/// the parser allocate without bound. Unknown formats are malformed tables
/// rather than empty coverage.
fn parse_coverage(
    data: &[u8],
    base: usize,
    offset: usize,
    num_glyphs: Option<u16>,
    what: &'static str,
) -> Result<Vec<u16>, ParseMathError> {
    let abs = base
        .checked_add(offset)
        .ok_or_else(|| ParseMathError::Malformed(format!("{what} coverage offset overflows")))?;
    let mut r = window(data, abs)?;
    let format = r.read_u16()?;
    match format {
        1 => {
            let count = r.read_u16()? as usize;
            if count > MAX_COVERAGE_GLYPHS {
                return Err(ParseMathError::LimitExceeded {
                    what: "coverage glyphs",
                    count,
                    max: MAX_COVERAGE_GLYPHS,
                });
            }
            let mut glyphs = Vec::with_capacity(count);
            for _ in 0..count {
                let glyph_id = r.read_u16()?;
                check_glyph(glyph_id, num_glyphs, what)?;
                glyphs.push(glyph_id);
            }
            Ok(glyphs)
        }
        2 => {
            let range_count = r.read_u16()? as usize;
            let mut glyphs = Vec::new();
            for _ in 0..range_count {
                let start_id = r.read_u16()?;
                let end_id = r.read_u16()?;
                let _start_coverage_index = r.read_u16()?;
                if end_id < start_id {
                    return Err(ParseMathError::Malformed(format!(
                        "{what} coverage range {start_id}..={end_id} is inverted"
                    )));
                }
                let span = end_id as usize - start_id as usize + 1;
                if glyphs.len() + span > MAX_COVERAGE_GLYPHS {
                    return Err(ParseMathError::LimitExceeded {
                        what: "coverage glyphs",
                        count: glyphs.len() + span,
                        max: MAX_COVERAGE_GLYPHS,
                    });
                }
                for glyph_id in start_id..=end_id {
                    check_glyph(glyph_id, num_glyphs, what)?;
                    glyphs.push(glyph_id);
                }
            }
            Ok(glyphs)
        }
        other => Err(ParseMathError::Malformed(format!("{what} coverage has unknown format {other}"))),
    }
}

// ---- MATH Header ----

/// MATH table version (0 and 1 are the only current versions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathVersion(pub u16);

impl MathVersion {
    pub const V0: MathVersion = MathVersion(0);
    pub const V1: MathVersion = MathVersion(1);
}

/// Top-level `MATH` table.
///
/// Header (MS-OTTO §3.1) — 10 bytes, three subtable offsets only:
/// ```text
/// majorVersion        u16
/// minorVersion        u16
/// mathConstantsOffset u16   (relative to the table start)
/// mathGlyphInfoOffset u16   (relative to the table start, 0 = absent)
/// mathVariantsOffset  u16   (relative to the table start, 0 = absent)
/// ```
/// There is no separate assembly offset: `GlyphAssembly` is reached through
/// each `MathGlyphConstruction`.
#[derive(Debug, Clone)]
pub struct MathTable {
    /// Table version (`1` in every shipping math font).
    pub version: MathVersion,
    /// The 56 numeric layout constants (required).
    pub constants: MathConstants,
    /// Italic correction, accent attachment, extended shapes and math kern.
    pub glyph_info: MathGlyphInfo,
    /// Horizontal and vertical delimiter constructions.
    pub variants: MathVariants,
}

impl MathTable {
    /// Parse a MATH table from its exact payload bytes, without glyph-range
    /// validation.
    pub fn parse(data: &[u8]) -> Result<Self, ParseMathError> {
        Self::parse_with_num_glyphs(data, None)
    }

    /// Parse a MATH table and validate every glyph ID it references against
    /// `num_glyphs` (from `maxp`). Pass `None` when the glyph count is unknown.
    pub fn parse_with_num_glyphs(data: &[u8], num_glyphs: Option<u16>) -> Result<Self, ParseMathError> {
        let mut r = Reader::new(data);
        let version = MathVersion(r.read_u16()?);
        let _minor = r.read_u16()?;
        if version.0 > 1 {
            return Err(ParseMathError::UnsupportedVersion(version.0));
        }
        let constants_off = r.read_u16()? as usize;
        let glyph_info_off = r.read_u16()? as usize;
        let variants_off = r.read_u16()? as usize;

        if constants_off == 0 {
            return Err(ParseMathError::Malformed("MathConstants is required but its offset is zero".into()));
        }
        let mut constants_r = window(data, constants_off)?;
        let constants = MathConstants::parse(&mut constants_r)?;

        let glyph_info = match subtable(data, 0, glyph_info_off, "MathGlyphInfo")? {
            Some(mut sub) => MathGlyphInfo::parse(&mut sub, num_glyphs)?,
            None => MathGlyphInfo::default(),
        };

        let variants = match subtable(data, 0, variants_off, "MathVariants")? {
            Some(mut sub) => MathVariants::parse(&mut sub, num_glyphs)?,
            None => MathVariants::default(),
        };

        Ok(Self { version, constants, glyph_info, variants })
    }
}

/// Errors while parsing `MATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseMathError {
    /// A read ran past the end of the table payload (truncated table).
    Bounds(ReadError),
    /// Structurally invalid table: impossible offset, unknown coverage format,
    /// inverted range, or a count that disagrees with its Coverage.
    Malformed(String),
    /// MATH major version outside the supported range (0 or 1).
    UnsupportedVersion(u16),
    /// A glyph ID at or above the font's `maxp.numGlyphs`.
    GlyphIdOutOfRange {
        /// The offending glyph ID.
        glyph_id: u16,
        /// Glyph count from `maxp`.
        num_glyphs: u16,
        /// Subtable that produced the ID.
        what: &'static str,
    },
    /// A count exceeded the hard cap enforced for that structure.
    LimitExceeded {
        /// Structure name.
        what: &'static str,
        /// Count found in the table.
        count: usize,
        /// Hard cap applied by this parser.
        max: usize,
    },
}

impl From<ReadError> for ParseMathError {
    #[inline]
    fn from(e: ReadError) -> Self {
        Self::Bounds(e)
    }
}

impl std::fmt::Display for ParseMathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bounds(e) => write!(f, "MATH bounds: {e}"),
            Self::Malformed(s) => write!(f, "MATH malformed: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "MATH unsupported version {v}"),
            Self::GlyphIdOutOfRange { glyph_id, num_glyphs, what } => {
                write!(f, "MATH {what} glyph id {glyph_id} outside font range (numGlyphs {num_glyphs})")
            }
            Self::LimitExceeded { what, count, max } => {
                write!(f, "MATH {what} count {count} exceeds limit {max}")
            }
        }
    }
}

impl std::error::Error for ParseMathError {}

// ---- Math Constants ----

/// `MathConstants` — all 56 fields of MS-OTTO §3.1, in specification order.
///
/// Layout is fixed and positional, so one wrong field shifts every later
/// value. The byte layout is:
/// ```text
///   4 simple fields                      8 bytes
///  51 MathValueRecord fields           204 bytes  (FWORD value + uint16 device offset)
///   1 uint16 RadicalDegreeBottomRaisePercent
///                                      ----------
///                                        214 bytes
/// ```
/// `ScriptPercentScaleDown` and `ScriptScriptPercentScaleDown` are `int16`,
/// `DelimitedSubFormulaMinHeight` and `DisplayOperatorMinHeight` are `uint16`,
/// `RadicalDegreeBottomRaisePercent` is `uint16`. All MathValueRecord values
/// are `int16` in font units, widened to `f32` here for layout arithmetic.
#[derive(Debug, Clone, PartialEq)]
pub struct MathConstants {
    /// ScriptPercentScaleDown (percent, e.g. 70).
    pub script_percent_scale_down: f32,
    /// ScriptScriptPercentScaleDown (percent).
    pub script_script_percent_scale_down: f32,
    /// DelimitedSubFormulaMinHeight (`uint16`, font units).
    pub delimited_sub_formula_min_height: f32,
    /// DisplayOperatorMinHeight (`uint16`, font units).
    pub display_operator_min_height: f32,

    // --- 51 MathValueRecords, specification order ---

    pub math_leading: f32,
    pub axis_height: f32,
    pub accent_base_height: f32,
    pub flattened_accent_base_height: f32,

    pub subscript_shift_down: f32,
    pub subscript_top_max: f32,
    pub subscript_baseline_drop_min: f32,

    pub superscript_shift_up: f32,
    pub superscript_shift_up_cramped: f32,
    pub superscript_bottom_min: f32,
    pub superscript_baseline_drop_max: f32,

    pub sub_superscript_gap_min: f32,
    pub superscript_bottom_max_with_subscript: f32,
    pub space_after_script: f32,

    pub upper_limit_gap_min: f32,
    pub upper_limit_baseline_rise_min: f32,
    pub lower_limit_gap_min: f32,
    pub lower_limit_baseline_drop_min: f32,

    pub stack_top_shift_up: f32,
    pub stack_top_display_style_shift_up: f32,
    pub stack_bottom_shift_down: f32,
    pub stack_bottom_display_style_shift_down: f32,
    pub stack_gap_min: f32,
    pub stack_display_style_gap_min: f32,

    pub stretch_stack_top_shift_up: f32,
    pub stretch_stack_bottom_shift_down: f32,
    pub stretch_stack_gap_above_min: f32,
    pub stretch_stack_gap_below_min: f32,

    pub fraction_numerator_shift_up: f32,
    pub fraction_numerator_display_style_shift_up: f32,
    pub fraction_denominator_shift_down: f32,
    pub fraction_denominator_display_style_shift_down: f32,
    pub fraction_numerator_gap_min: f32,
    pub fraction_num_display_style_gap_min: f32,
    pub fraction_rule_thickness: f32,
    pub fraction_denominator_gap_min: f32,
    pub fraction_denom_display_style_gap_min: f32,

    pub skewed_fraction_horizontal_gap: f32,
    pub skewed_fraction_vertical_gap: f32,

    pub overbar_vertical_gap: f32,
    pub overbar_rule_thickness: f32,
    pub overbar_extra_ascender: f32,

    pub underbar_vertical_gap: f32,
    pub underbar_rule_thickness: f32,
    pub underbar_extra_descender: f32,

    pub radical_vertical_gap: f32,
    pub radical_display_style_vertical_gap: f32,
    pub radical_rule_thickness: f32,
    pub radical_extra_ascender: f32,
    pub radical_kern_before_degree: f32,
    pub radical_kern_after_degree: f32,

    /// RadicalDegreeBottomRaisePercent (`uint16`, percent).
    pub radical_degree_bottom_raise_percent: f32,
}

impl MathConstants {
    /// Byte length of one `MathConstants` record: 4 simple fields (8 bytes),
    /// 51 MathValueRecords (204 bytes) and one `uint16` (2 bytes).
    pub const BYTE_LEN: usize = 214;

    /// Parse MathConstants from the start of the sub-table.
    ///
    /// The fields are read strictly in MS-OTTO §3.1 order (the same order they
    /// are declared in [`MathConstants`]). MathValueRecords are 4 bytes each:
    /// an `int16` value in font units followed by a `uint16` device-table
    /// offset. The device offset is hinting metadata, not a font fact, so only
    /// the value is kept.
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ParseMathError> {
        fn mvr(r: &mut Reader<'_>) -> Result<f32, ReadError> {
            let value = r.read_fword()?;
            let _device_table_offset = r.read_u16()?;
            Ok(value)
        }

        Ok(Self {
            // --- 4 simple fields ---
            script_percent_scale_down: r.read_i16()? as f32,
            script_script_percent_scale_down: r.read_i16()? as f32,
            delimited_sub_formula_min_height: r.read_ufword()?,
            display_operator_min_height: r.read_ufword()?,

            // --- 51 MathValueRecords ---
            math_leading: mvr(r)?,
            axis_height: mvr(r)?,
            accent_base_height: mvr(r)?,
            flattened_accent_base_height: mvr(r)?,

            subscript_shift_down: mvr(r)?,
            subscript_top_max: mvr(r)?,
            subscript_baseline_drop_min: mvr(r)?,

            superscript_shift_up: mvr(r)?,
            superscript_shift_up_cramped: mvr(r)?,
            superscript_bottom_min: mvr(r)?,
            superscript_baseline_drop_max: mvr(r)?,

            sub_superscript_gap_min: mvr(r)?,
            superscript_bottom_max_with_subscript: mvr(r)?,
            space_after_script: mvr(r)?,

            upper_limit_gap_min: mvr(r)?,
            upper_limit_baseline_rise_min: mvr(r)?,
            lower_limit_gap_min: mvr(r)?,
            lower_limit_baseline_drop_min: mvr(r)?,

            stack_top_shift_up: mvr(r)?,
            stack_top_display_style_shift_up: mvr(r)?,
            stack_bottom_shift_down: mvr(r)?,
            stack_bottom_display_style_shift_down: mvr(r)?,
            stack_gap_min: mvr(r)?,
            stack_display_style_gap_min: mvr(r)?,

            stretch_stack_top_shift_up: mvr(r)?,
            stretch_stack_bottom_shift_down: mvr(r)?,
            stretch_stack_gap_above_min: mvr(r)?,
            stretch_stack_gap_below_min: mvr(r)?,

            fraction_numerator_shift_up: mvr(r)?,
            fraction_numerator_display_style_shift_up: mvr(r)?,
            fraction_denominator_shift_down: mvr(r)?,
            fraction_denominator_display_style_shift_down: mvr(r)?,
            fraction_numerator_gap_min: mvr(r)?,
            fraction_num_display_style_gap_min: mvr(r)?,
            fraction_rule_thickness: mvr(r)?,
            fraction_denominator_gap_min: mvr(r)?,
            fraction_denom_display_style_gap_min: mvr(r)?,

            skewed_fraction_horizontal_gap: mvr(r)?,
            skewed_fraction_vertical_gap: mvr(r)?,

            overbar_vertical_gap: mvr(r)?,
            overbar_rule_thickness: mvr(r)?,
            overbar_extra_ascender: mvr(r)?,

            underbar_vertical_gap: mvr(r)?,
            underbar_rule_thickness: mvr(r)?,
            underbar_extra_descender: mvr(r)?,

            radical_vertical_gap: mvr(r)?,
            radical_display_style_vertical_gap: mvr(r)?,
            radical_rule_thickness: mvr(r)?,
            radical_extra_ascender: mvr(r)?,
            radical_kern_before_degree: mvr(r)?,
            radical_kern_after_degree: mvr(r)?,

            // --- trailing uint16 ---
            radical_degree_bottom_raise_percent: r.read_ufword()?,
        })
    }
}

// ---- MathGlyphInfo ----

/// One `MathKern` subtable — kern values keyed by correction height.
///
/// `kern_values.len()` is always `correction_heights.len() + 1`; the extra
/// entry applies above the tallest recorded correction height.
#[derive(Debug, Clone, PartialEq)]
pub struct MathKernTable {
    /// Correction heights in font units, ascending.
    pub correction_heights: Vec<f32>,
    /// Kern values in font units, one per height boundary plus the open end.
    pub kern_values: Vec<f32>,
}

impl MathKernTable {
    /// Parse one `MathKern` subtable located at `base + offset`.
    fn parse_from(data: &[u8], base: usize, offset: usize, what: &'static str) -> Result<Self, ParseMathError> {
        let abs = base
            .checked_add(offset)
            .ok_or_else(|| ParseMathError::Malformed(format!("{what} offset overflows")))?;
        let mut r = window(data, abs)?;
        let height_count = r.read_u16()? as usize;
        if height_count > MAX_KERN_HEIGHTS {
            return Err(ParseMathError::LimitExceeded { what: "kern heights", count: height_count, max: MAX_KERN_HEIGHTS });
        }
        let mut correction_heights = Vec::with_capacity(height_count);
        for _ in 0..height_count {
            correction_heights.push(read_math_value(&mut r)?);
        }
        let mut kern_values = Vec::with_capacity(height_count + 1);
        for _ in 0..=height_count {
            kern_values.push(read_math_value(&mut r)?);
        }
        Ok(Self { correction_heights, kern_values })
    }
}

/// `MathGlyphInfo` — glyph-indexed math metrics.
///
/// Layout (MS-OTTO §3.2), every offset relative to the start of
/// `MathGlyphInfo`:
/// ```text
/// MathItalicsCorrectionInfoOffset  u16
/// MathTopAccentAttachmentOffset    u16
/// ExtendedShapeCoverageOffset      u16
/// MathKernInfoOffset               u16
/// ```
/// Each subtable is Coverage-parallel: `coverage[i]` owns record `i`.
#[derive(Debug, Clone, Default)]
pub struct MathGlyphInfo {
    /// Italic correction per covered glyph, in font units.
    pub italic_correction: HashMap<u16, f32>,
    /// Horizontal top-accent attachment point per covered glyph, in font units.
    pub top_accent_attachment: HashMap<u16, f32>,
    /// Glyphs flagged as extended shapes.
    pub extended_shapes: Vec<u16>,
    /// Kern data for the top-right corner per covered glyph.
    pub math_kern_top_right: HashMap<u16, MathKernTable>,
    /// Kern data for the top-left corner per covered glyph.
    pub math_kern_top_left: HashMap<u16, MathKernTable>,
    /// Kern data for the bottom-right corner per covered glyph.
    pub math_kern_bottom_right: HashMap<u16, MathKernTable>,
    /// Kern data for the bottom-left corner per covered glyph.
    pub math_kern_bottom_left: HashMap<u16, MathKernTable>,
}

impl MathGlyphInfo {
    /// Parse `MathGlyphInfo`; `r` must start at the subtable.
    pub fn parse(r: &mut Reader<'_>, num_glyphs: Option<u16>) -> Result<Self, ParseMathError> {
        let base = r.position();
        let italic_info_off = r.read_u16()? as usize;
        let accent_info_off = r.read_u16()? as usize;
        let shape_cov_off = r.read_u16()? as usize;
        let kern_info_off = r.read_u16()? as usize;
        let data = r.bytes;

        let mut info = Self::default();

        // MathItalicsCorrectionInfo: Coverage + count + one MVR per glyph.
        if let Some(mut sub) = subtable(data, base, italic_info_off, "MathItalicsCorrectionInfo")? {
            let sub_base = sub.position();
            let coverage_off = sub.read_u16()? as usize;
            let count = sub.read_u16()? as usize;
            let glyphs = parse_coverage(data, sub_base, coverage_off, num_glyphs, "MathItalicsCorrectionInfo")?;
            if count != glyphs.len() {
                return Err(ParseMathError::Malformed(format!(
                    "MathItalicsCorrectionInfo count {count} disagrees with coverage {}",
                    glyphs.len()
                )));
            }
            for glyph_id in glyphs {
                info.italic_correction.insert(glyph_id, read_math_value(&mut sub)?);
            }
        }

        // MathTopAccentAttachment: Coverage + count + one MVR per glyph. The
        // value is the horizontal position of the accent attachment point.
        if let Some(mut sub) = subtable(data, base, accent_info_off, "MathTopAccentAttachment")? {
            let sub_base = sub.position();
            let coverage_off = sub.read_u16()? as usize;
            let count = sub.read_u16()? as usize;
            let glyphs = parse_coverage(data, sub_base, coverage_off, num_glyphs, "MathTopAccentAttachment")?;
            if count != glyphs.len() {
                return Err(ParseMathError::Malformed(format!(
                    "MathTopAccentAttachment count {count} disagrees with coverage {}",
                    glyphs.len()
                )));
            }
            for glyph_id in glyphs {
                info.top_accent_attachment.insert(glyph_id, read_math_value(&mut sub)?);
            }
        }

        // ExtendedShapeCoverage is a bare Coverage table.
        if shape_cov_off != 0 {
            info.extended_shapes = parse_coverage(data, base, shape_cov_off, num_glyphs, "ExtendedShapeCoverage")?;
        }

        // MathKernInfo: Coverage + count + four Offset16 per glyph in
        // top-right, top-left, bottom-right, bottom-left order. All offsets are
        // relative to the MathKernInfo start. A zero offset means that corner
        // has no kern data for that glyph.
        if let Some(mut sub) = subtable(data, base, kern_info_off, "MathKernInfo")? {
            let sub_base = sub.position();
            let coverage_off = sub.read_u16()? as usize;
            let count = sub.read_u16()? as usize;
            let glyphs = parse_coverage(data, sub_base, coverage_off, num_glyphs, "MathKernInfo")?;
            if count != glyphs.len() {
                return Err(ParseMathError::Malformed(format!(
                    "MathKernInfo count {count} disagrees with coverage {}",
                    glyphs.len()
                )));
            }
            for glyph_id in glyphs {
                let top_right = sub.read_u16()? as usize;
                let top_left = sub.read_u16()? as usize;
                let bottom_right = sub.read_u16()? as usize;
                let bottom_left = sub.read_u16()? as usize;
                insert_kern(&mut info.math_kern_top_right, data, sub_base, top_right, glyph_id, "MathKernTopRight")?;
                insert_kern(&mut info.math_kern_top_left, data, sub_base, top_left, glyph_id, "MathKernTopLeft")?;
                insert_kern(
                    &mut info.math_kern_bottom_right,
                    data,
                    sub_base,
                    bottom_right,
                    glyph_id,
                    "MathKernBottomRight",
                )?;
                insert_kern(
                    &mut info.math_kern_bottom_left,
                    data,
                    sub_base,
                    bottom_left,
                    glyph_id,
                    "MathKernBottomLeft",
                )?;
            }
        }

        Ok(info)
    }
}

/// Parse one corner `MathKern` at `base + offset` and store it under
/// `glyph_id`. A zero offset means the font has no kern data for that corner.
fn insert_kern(
    map: &mut HashMap<u16, MathKernTable>,
    data: &[u8],
    base: usize,
    offset: usize,
    glyph_id: u16,
    what: &'static str,
) -> Result<(), ParseMathError> {
    if offset != 0 {
        map.insert(glyph_id, MathKernTable::parse_from(data, base, offset, what)?);
    }
    Ok(())
}

// ---- MathVariants ----

/// One `MathGlyphVariantRecord`: a prebuilt larger glyph and its advance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathVariantRecord {
    /// Variant glyph ID.
    pub variant_glyph: u16,
    /// Advance measurement along the construction axis, in font units.
    pub advance_measurement: f32,
}

/// One part of a `GlyphAssembly` recipe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartRecord {
    /// Glyph ID of the part.
    pub glyph_id: u16,
    /// Length of the connector at the start of the part, in font units.
    pub start_connector_length: f32,
    /// Length of the connector at the end of the part, in font units.
    pub end_connector_length: f32,
    /// Full advance of the part along the construction axis, in font units.
    pub full_advance: f32,
    /// `PartFlags`: bit 0 is `PartFlagsExtender`, the rest are reserved.
    pub part_flags: u16,
}

impl PartRecord {
    /// `PartFlagsExtender` — the part may be repeated to fill extra length.
    pub const EXTENDER: u16 = 0x0001;
}

/// A `GlyphAssembly`: how to build an arbitrarily tall or wide glyph by
/// repeating and joining parts.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphAssembly {
    /// Italic correction of the assembled glyph, in font units.
    pub italics_correction: f32,
    /// Ordered parts, smallest repeatable piece first.
    pub parts: Vec<PartRecord>,
}

impl GlyphAssembly {
    /// Parse a `GlyphAssembly`; `r` must start at the assembly.
    fn parse(r: &mut Reader<'_>, num_glyphs: Option<u16>) -> Result<Self, ParseMathError> {
        let italics_correction = read_math_value(r)?;
        let part_count = r.read_u16()? as usize;
        if part_count > MAX_ASSEMBLY_PARTS {
            return Err(ParseMathError::LimitExceeded {
                what: "assembly parts",
                count: part_count,
                max: MAX_ASSEMBLY_PARTS,
            });
        }
        let mut parts = Vec::with_capacity(part_count);
        for _ in 0..part_count {
            let glyph_id = r.read_u16()?;
            check_glyph(glyph_id, num_glyphs, "GlyphAssembly")?;
            parts.push(PartRecord {
                glyph_id,
                start_connector_length: r.read_ufword()?,
                end_connector_length: r.read_ufword()?,
                full_advance: r.read_ufword()?,
                part_flags: r.read_u16()?,
            });
        }
        Ok(Self { italics_correction, parts })
    }
}

/// One `MathGlyphConstruction`: the construction records for one covered glyph.
///
/// Layout (MS-OTTO §3.3), offsets relative to the construction start:
/// ```text
/// GlyphAssemblyOffset  u16   (0 = no assembly)
/// VariantCount         u16
/// MathGlyphVariantRecord[VariantCount]   (VariantGlyph u16, AdvanceMeasurement u16)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MathGlyphConstruction {
    /// Glyph this construction applies to.
    pub glyph_id: u16,
    /// Prebuilt larger variants, smallest first.
    pub variants: Vec<MathVariantRecord>,
    /// Recipe for sizes beyond the last variant, when the font provides one.
    pub assembly: Option<GlyphAssembly>,
}

impl MathGlyphConstruction {
    /// Parse a construction; `r` must start at the construction.
    fn parse(r: &mut Reader<'_>, glyph_id: u16, num_glyphs: Option<u16>) -> Result<Self, ParseMathError> {
        let base = r.position();
        let assembly_off = r.read_u16()? as usize;
        let variant_count = r.read_u16()? as usize;
        if variant_count > MAX_VARIANTS_PER_CONSTRUCTION {
            return Err(ParseMathError::LimitExceeded {
                what: "MathGlyphConstruction variants",
                count: variant_count,
                max: MAX_VARIANTS_PER_CONSTRUCTION,
            });
        }
        let mut variants = Vec::with_capacity(variant_count);
        for _ in 0..variant_count {
            let variant_glyph = r.read_u16()?;
            check_glyph(variant_glyph, num_glyphs, "MathGlyphConstruction")?;
            variants.push(MathVariantRecord { variant_glyph, advance_measurement: r.read_ufword()? });
        }
        let data = r.bytes;
        let assembly = match subtable(data, base, assembly_off, "GlyphAssembly")? {
            Some(mut sub) => Some(GlyphAssembly::parse(&mut sub, num_glyphs)?),
            None => None,
        };
        Ok(Self { glyph_id, variants, assembly })
    }
}

/// `MathVariants` — delimiter and large-operator scaling data.
///
/// Layout (MS-OTTO §3.3), all offsets relative to the `MathVariants` start:
/// ```text
/// MinConnectorOverlap       u16
/// VertGlyphCoverageOffset   u16
/// HorizGlyphCoverageOffset  u16
/// VertGlyphCount            u16
/// HorizGlyphCount           u16
/// VertGlyphConstruction[VertGlyphCount]   Offset16[]
/// HorizGlyphConstruction[HorizGlyphCount] Offset16[]
/// ```
/// `VertGlyphCount` and `HorizGlyphCount` must equal their Coverage lengths:
/// the offset arrays are parallel to the coverage glyph lists.
#[derive(Debug, Clone, Default)]
pub struct MathVariants {
    /// Minimum overlap between adjacent assembly parts, in font units.
    pub min_connector_overlap: f32,
    /// Glyphs with a vertical construction, in coverage order.
    pub vert_coverages: Vec<u16>,
    /// Glyphs with a horizontal construction, in coverage order.
    pub horiz_coverages: Vec<u16>,
    /// Vertical constructions indexed by glyph ID.
    pub vert_constructions: HashMap<u16, MathGlyphConstruction>,
    /// Horizontal constructions indexed by glyph ID.
    pub horiz_constructions: HashMap<u16, MathGlyphConstruction>,
}

impl MathVariants {
    /// Parse `MathVariants`; `r` must start at the subtable.
    pub fn parse(r: &mut Reader<'_>, num_glyphs: Option<u16>) -> Result<Self, ParseMathError> {
        let base = r.position();
        let min_connector_overlap = r.read_ufword()?;
        let vert_cov_off = r.read_u16()? as usize;
        let horiz_cov_off = r.read_u16()? as usize;
        let vert_count = r.read_u16()? as usize;
        let horiz_count = r.read_u16()? as usize;
        let data = r.bytes;

        let vert_coverages = if vert_cov_off == 0 {
            Vec::new()
        }
        else {
            parse_coverage(data, base, vert_cov_off, num_glyphs, "VertGlyphCoverage")?
        };
        let horiz_coverages = if horiz_cov_off == 0 {
            Vec::new()
        }
        else {
            parse_coverage(data, base, horiz_cov_off, num_glyphs, "HorizGlyphCoverage")?
        };
        if vert_count != vert_coverages.len() {
            return Err(ParseMathError::Malformed(format!(
                "VertGlyphCount {vert_count} disagrees with VertGlyphCoverage {}",
                vert_coverages.len()
            )));
        }
        if horiz_count != horiz_coverages.len() {
            return Err(ParseMathError::Malformed(format!(
                "HorizGlyphCount {horiz_count} disagrees with HorizGlyphCoverage {}",
                horiz_coverages.len()
            )));
        }

        let mut vert_constructions = HashMap::with_capacity(vert_count);
        for &glyph_id in &vert_coverages {
            let off = r.read_u16()? as usize;
            let mut sub = subtable(data, base, off, "VertGlyphConstruction")?.ok_or_else(|| {
                ParseMathError::Malformed(format!("VertGlyphConstruction for glyph {glyph_id} has a zero offset"))
            })?;
            vert_constructions.insert(glyph_id, MathGlyphConstruction::parse(&mut sub, glyph_id, num_glyphs)?);
        }

        let mut horiz_constructions = HashMap::with_capacity(horiz_count);
        for &glyph_id in &horiz_coverages {
            let off = r.read_u16()? as usize;
            let mut sub = subtable(data, base, off, "HorizGlyphConstruction")?.ok_or_else(|| {
                ParseMathError::Malformed(format!("HorizGlyphConstruction for glyph {glyph_id} has a zero offset"))
            })?;
            horiz_constructions.insert(glyph_id, MathGlyphConstruction::parse(&mut sub, glyph_id, num_glyphs)?);
        }

        Ok(Self { min_connector_overlap, vert_coverages, horiz_coverages, vert_constructions, horiz_constructions })
    }
}
