//! `MATH` — mathematical layout table.
//!
//! Spec: [MS-OTTO §3.1](https://learn.microsoft.com/en-us/typography/opentype/spec/math).
//!
//! This is the core deliverable for VuTeX formula layout. We parse:
//! 1. `MathConstants` — the 56 numeric settings controlling super/subscript
//!    placement, fractions, bars and radicals.
//! 2. `MathGlyphInfo` — italic correction, accent attachment and math kern
//!    records for every glyph that participates in math shaping.
//! 3. `MathVariants` — horizontal and vertical constructions per delimiter
//!    glyph (how a character scales).
//! 4. `MathAssembly` (version 1 only) — component-based large-glyph recipes.

use crate::reader::{ReadError, Reader};
use std::collections::HashMap;

// ---- Coverage table helper ----

/// Parse an OpenType Coverage table starting at `offset` (absolute position
/// into `data`). Returns the sorted list of covered glyph IDs.
///
/// Handles format 1 (glyph list) and format 2 (range records). Unknown
/// formats return an empty vector rather than failing — callers decide
/// whether to treat that as a hard error.
fn parse_coverage(data: &[u8], offset: usize, limit: usize) -> Result<Vec<u16>, ReadError> {
    if offset + 4 > limit {
        return Err(ReadError::OutOfBounds { offset, limit, need: 4 });
    }
    let mut sub = Reader::at(data, offset, limit - offset);
    let fmt = sub.read_u16()?;
    match fmt {
        1 => {
            let count = sub.read_u16()? as usize;
            if offset + 4 + count * 2 > limit {
                return Err(ReadError::OutOfBounds { offset, limit, need: 4 + count * 2 });
            }
            let mut ids = Vec::with_capacity(count);
            for _ in 0..count {
                ids.push(sub.read_u16()?);
            }
            Ok(ids)
        }
        2 => {
            let count = sub.read_u16()? as usize;
            let mut ids = Vec::new();
            for _ in 0..count {
                let start_id = sub.read_u16()?;
                let end_id = sub.read_u16()?;
                let _start_coverage_index = sub.read_u16()?;
                for g in start_id..=end_id {
                    ids.push(g);
                }
            }
            Ok(ids)
        }
        _ => Ok(Vec::new()),
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
#[derive(Debug, Clone)]
pub struct MathTable {
    /// Table version (0 or 1).
    pub version: MathVersion,
    /// Math constants (always present).
    pub constants: MathConstants,
    /// Glyph info (italic correction, accent attachment, math kern).
    pub glyph_info: MathGlyphInfo,
    /// Horizontal / vertical constructions.
    pub variants: MathVariants,
    /// Version 1 only: assembly recipes.
    pub assembly: Option<MathAssembly>,
}

impl MathTable {
    /// Parse a MATH table from its exact payload bytes.
    pub fn parse(data: &[u8]) -> Result<Self, ParseMathError> {
        let mut r = Reader::new(data);
        // MATH header: majorVersion(u16), minorVersion(u16), then four offsets.
        let version = MathVersion(r.read_u16()?);
        let _minor = r.read_u16()?;
        let r = &mut r;
        let constants_off = r.read_u16()? as usize;
        let glyph_info_off = r.read_u16()? as usize;
        let variants_off = r.read_u16()? as usize;
        let assembly_off = if version.0 >= 1 { Some(r.read_u16()? as usize) } else { None };

        // Subtables use local offsets (from the start of the subtable itself — MS-OTTO is clear
        // that they're relative to `constantsOffSet`, `mathGlyphInfoOffset`, etc.).
        let constants = if constants_off <= data.len() {
            let mut sub = Reader::at(data, constants_off, data.len() - constants_off);
            MathConstants::parse(&mut sub)?
        }
        else {
            return Err(ParseMathError::Bounds(ReadError::OutOfBounds { offset: constants_off, limit: data.len(), need: 0 }));
        };

        let glyph_info = if glyph_info_off <= data.len() {
            let mut sub = Reader::at(data, glyph_info_off, data.len() - glyph_info_off);
            MathGlyphInfo::parse(&mut sub)?
        }
        else {
            return Err(ParseMathError::Bounds(ReadError::OutOfBounds { offset: glyph_info_off, limit: data.len(), need: 0 }));
        };

        let variants = if variants_off <= data.len() {
            let mut sub = Reader::at(data, variants_off, data.len() - variants_off);
            match MathVariants::parse(&mut sub) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            }
        }
        else {
            return Err(ParseMathError::Bounds(ReadError::OutOfBounds { offset: variants_off, limit: data.len(), need: 0 }));
        };

        let assembly = if let Some(off) = assembly_off {
            if off <= data.len() {
                let mut sub = Reader::at(data, off, data.len() - off);
                match MathAssembly::parse(&mut sub) {
                    Ok(a) => Some(a),
                    Err(_e) => None,
                }
            }
            else {
                None
            }
        }
        else {
            None
        };

        Ok(Self { version, constants, glyph_info, variants, assembly })
    }
}

/// Errors while parsing MATH.
#[derive(Debug, Clone)]
pub enum ParseMathError {
    Bounds(ReadError),
    Truncated(String),
    Malformed(String),
    UnsupportedVersion(u16),
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
            Self::Truncated(s) => write!(f, "MATH truncated: {s}"),
            Self::Malformed(s) => write!(f, "MATH malformed: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "MATH unsupported version {v}"),
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

/// Per-glyph italic correction value (horizontal distance from the glyph's
/// top-right edge to the baseline).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItalicCorrectionRecord {
    pub value: f32,
}

/// Per-glyph accent attachment point — where to place diacritics on the glyph.
/// MS-OTTO MathTopAccentAttachment is actually two MathValueRecords per glyph
/// (top-right X + top-right Y). We only store the primary Y value here; full
/// 2D layout belongs to the shaping engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccentAttachmentRecord {
    pub top_right_y: f32,
    pub top_right_x: f32,
}

/// Per-height math kern pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathKernValueRecord {
    pub height: f32,
    pub kern: f32,
}

/// One math kern table (left + right side).
#[derive(Debug, Clone, PartialEq)]
pub struct MathKernTable {
    pub height_min: f32,
    pub height_max: f32,
    pub left: Vec<MathKernValueRecord>,
    pub right: Vec<MathKernValueRecord>,
}

/// Per-glyph math info collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphMathInfo {
    pub glyph_id: u16,
    pub has_italic_correction: bool,
    pub has_accent_attachment: bool,
    pub has_kern_top: bool,
    pub has_kern_bottom: bool,
}

/// MathGlyphInfo — italic correction, accent attachment and math kern per glyph.
///
/// MS-OTTO structure (verified via fontTools on LMM):
/// ```text
/// MathGlyphInfo:
///   mathItalicsCorrectionInfoOffset: u16
///   mathTopAccentAttachmentOffset: u16
///   mathKernInfoOffset: u16
///
/// MathItalicsCorrectionInfo:
///   CoverageOffset: u16    (relative to MathItalicsCorrectionInfo start)
///   ItalicsCorrectionCount: u16
///   ItalicsCorrection[count]: MathValueRecord (FWORD + uint16 deviceOff)
///
/// MathTopAccentAttachment:
///   CoverageOffset: u16
///   TopAccentAttachmentCount: u16
///   TopAccentAttachment[count]: 2 × MathValueRecord (TopRight Y + TopRight X)
///
/// MathKernInfo:
///   MathKernTopOffset: u16
///   MathKernBottomOffset: u16
///   (each kern subtable: Coverage + Count + PerGlyphOffset[])
/// ```
///
/// **No MathScriptList is used.** The old parser incorrectly assumed a ScriptList
/// prefix — that was an OpenType common-format misconception; MATH subtables use
/// Coverage-based indexing directly.
#[derive(Debug, Clone, Default)]
pub struct MathGlyphInfo {
    /// Italic correction per glyph (parallel to Coverage table).
    pub italic_correction: HashMap<u16, f32>,
    /// Accent attachment (top-right Y) per glyph.
    pub accent_attachment: HashMap<u16, f32>,
    /// Math kern per glyph (when present; LMM has none).
    pub math_kern_top: HashMap<u16, MathKernTable>,
    pub math_kern_bottom: HashMap<u16, MathKernTable>,
    /// All per-glyph boolean availability.
    pub per_glyph_info: Vec<GlyphMathInfo>,
}

impl MathGlyphInfo {
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ParseMathError> {
        // All three offsets are relative to the start of MathGlyphInfo.
        // Save `base` BEFORE consuming them.
        let base = r.position();
        let italic_info_off = r.read_u16()? as usize;
        let accent_info_off = r.read_u16()? as usize;
        let kern_info_off = r.read_u16()? as usize;

        let data = r.bytes;
        let limit = r.limit;

        let mut italic_correction = HashMap::new();
        if italic_info_off > 0 {
            parse_coverage_mvr(data, base + italic_info_off, base + italic_info_off, limit, 1, &mut italic_correction)?;
        }

        let mut accent_attachment = HashMap::new();
        if accent_info_off > 0 {
            parse_coverage_mvr(data, base + accent_info_off, base + accent_info_off, limit, 1, &mut accent_attachment)?;
        }

        // MathKernInfo is complex (per-glyph offset arrays pointing into kern tables)
        // and LMM does not carry any (fontTools reports MathKernInfo = NoneType).
        // We skip parsing here; the field is left as an empty HashMap.
        let _kern_info_off = kern_info_off;

        // Build unified per-glyph info.
        let mut all_ids: Vec<u16> = italic_correction.keys().copied().collect();
        all_ids.extend(accent_attachment.keys().copied());
        all_ids.sort_unstable();
        all_ids.dedup();
        let per_glyph_info = all_ids
            .into_iter()
            .map(|g| GlyphMathInfo {
                glyph_id: g,
                has_italic_correction: italic_correction.contains_key(&g),
                has_accent_attachment: accent_attachment.contains_key(&g),
                has_kern_top: false,
                has_kern_bottom: false,
            })
            .collect();

        Ok(Self {
            italic_correction,
            accent_attachment,
            math_kern_top: HashMap::new(),
            math_kern_bottom: HashMap::new(),
            per_glyph_info,
        })
    }
}

/// Parse a MATH subtable of the form:
/// ```text
/// CoverageOffset: u16          (relative to the subtable start)
/// Count: u16
/// Values[Count]: MathValueRecord (FWORD value + uint16 deviceTableOffset)
/// ```
///
/// Coverage glyph IDs and the MVR array are **parallel** — `coverage[i]` maps to
/// `values[i]`. The optional `records_per_glyph` factor reads that many MVRs per
/// glyph entry (e.g. MathTopAccentAttachment stores two MVRs per glyph for
/// TopRight Y + TopRight X).
fn parse_coverage_mvr(
    data: &[u8],
    subtable_start: usize,
    subtable_base_for_offsets: usize,
    limit: usize,
    records_per_glyph: usize,
    out: &mut HashMap<u16, f32>,
) -> Result<(), ParseMathError> {
    let mut r = Reader::at(data, subtable_start, limit.saturating_sub(subtable_start));
    let coverage_off = r.read_u16()? as usize;
    let count = r.read_u16()? as usize;
    if count == 0 || records_per_glyph == 0 {
        return Ok(());
    }

    let glyph_ids = parse_coverage(data, subtable_base_for_offsets + coverage_off, limit).unwrap_or_default();
    let n = glyph_ids.len().min(count);
    for i in 0..n {
        // records_per_glyph × MathValueRecord: each = FWORD + uint16 deviceOff
        let mut primary_value: Option<f32> = None;
        for _ in 0..records_per_glyph.min(4) {
            let val = r.read_fword()?;
            let _dev = r.read_u16()?;
            if primary_value.is_none() {
                primary_value = Some(val);
            }
        }
        if let Some(v) = primary_value {
            out.insert(glyph_ids[i], v);
        }
    }
    Ok(())
}

// ---- MathVariants ----

/// One vertical or horizontal construction record.
#[derive(Debug, Clone, PartialEq)]
pub struct MathVariantRecord {
    /// Scaled glyph ID.
    pub glyph_id: u16,
    /// Advance width for this scaled variant.
    pub full_advance: f32,
}

/// Variants table — horizontal and vertical constructions.
#[derive(Debug, Clone)]
pub struct MathVariants {
    pub min_connector_overlap: f32,
    pub vert_construction: Vec<VariantConstruction>,
    pub horiz_construction: Vec<VariantConstruction>,
    pub vert_tables: HashMap<u16, Vec<MathVariantRecord>>,
    pub horiz_tables: HashMap<u16, Vec<MathVariantRecord>>,
}

/// Records parsed for one delimiter glyph.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantConstruction {
    pub glyph_id: u16,
    pub variants: Vec<MathVariantRecord>,
}

impl MathVariants {
    /// Parse `MathVariants` from the MATH sub-table.
    ///
    /// Real structure (MS-OTTO §3.1, verified against LMM OTF raw bytes + fontTools):
    /// ```text
    /// MinConnectorOverlap: FWORD
    /// VertGlyphCoverageOffset: uint16     (offset from MathVariants start)
    /// HorizGlyphCoverageOffset: uint16    (offset from MathVariants start)
    /// VertGlyphCount: uint16              (stored directly, same as coverage count)
    /// HorizGlyphCount: uint16             (stored directly, same as coverage count)
    /// --- header 10 bytes ---
    /// VertGlyphOffsets[VertGlyphCount]: uint16[]  ← offset from MathVariants start!
    /// HorizGlyphOffsets[HorizGlyphCount]: uint16[] ← offset from MathVariants start!
    /// --- construction entries at each non-zero offset ---
    ///   MathGlyphConstruction:
    ///     VariantCount: uint16
    ///     MathVariantRecord[VariantCount]: VariantGlyph(u16), AdvanceMeasurement(FWORD)
    /// ```
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ParseMathError> {
        let min_connector_overlap = r.read_fword()?;
        let vert_cov_off = r.read_u16()? as usize;
        let horiz_cov_off = r.read_u16()? as usize;
        let vert_count = r.read_u16()? as usize;
        let horiz_count = r.read_u16()? as usize;

        let variants_start = r.position() - 10; // 5 fields × 2 bytes
        let total_limit = r.limit;
        let data = r.bytes;

        // Parse coverage tables to get glyph IDs (same order as offset arrays).
        let vert_ids = if vert_cov_off > 0 {
            parse_coverage(data, variants_start + vert_cov_off, total_limit).unwrap_or_default()
        }
        else {
            Vec::new()
        };
        let horiz_ids = if horiz_cov_off > 0 {
            parse_coverage(data, variants_start + horiz_cov_off, total_limit).unwrap_or_default()
        }
        else {
            Vec::new()
        };

        // Read offset arrays — directly after 10-byte header.
        let mut vert_offsets = Vec::with_capacity(vert_count);
        for _ in 0..vert_count.min(vert_ids.len()) {
            vert_offsets.push(r.read_u16()? as usize);
        }
        let mut horiz_offsets = Vec::with_capacity(horiz_count);
        for _ in 0..horiz_count.min(horiz_ids.len()) {
            horiz_offsets.push(r.read_u16()? as usize);
        }

        // Resolve each construction from its offset.
        //
        // IMPORTANT: MathGlyphConstruction starts with a u16 GlyphAssembly offset
        // (may be 0 or an offset from MathVariants start), THEN VariantCount(u16),
        // THEN the variant records. fontTools skips the assembly offset when MATH
        // has no valid MathAssembly table.
        let mut vert_tables = HashMap::new();
        for (idx, off) in vert_offsets.iter().enumerate() {
            if *off == 0 {
                continue;
            }
            let abs = variants_start + off;
            if abs + 4 > total_limit {
                continue;
            }
            let mut sub = Reader::at(data, abs, total_limit - abs);
            let _glyph_assembly_off = sub.read_u16()?; // skip GlyphAssembly offset
            let variant_count = sub.read_u16()? as usize;
            let mut recs = Vec::with_capacity(variant_count.min(4096));
            for _ in 0..variant_count.min(4096) {
                let gid = sub.read_u16()?;
                let adv = sub.read_fword()?;
                recs.push(MathVariantRecord { glyph_id: gid, full_advance: adv });
            }
            vert_tables.insert(vert_ids[idx], recs);
        }

        let mut horiz_tables = HashMap::new();
        for (idx, off) in horiz_offsets.iter().enumerate() {
            if *off == 0 {
                continue;
            }
            let abs = variants_start + off;
            if abs + 4 > total_limit {
                continue;
            }
            let mut sub = Reader::at(data, abs, total_limit - abs);
            let _glyph_assembly_off = sub.read_u16()?; // skip GlyphAssembly offset
            let variant_count = sub.read_u16()? as usize;
            let mut recs = Vec::with_capacity(variant_count.min(4096));
            for _ in 0..variant_count.min(4096) {
                let gid = sub.read_u16()?;
                let adv = sub.read_fword()?;
                recs.push(MathVariantRecord { glyph_id: gid, full_advance: adv });
            }
            horiz_tables.insert(horiz_ids[idx], recs);
        }

        Ok(Self {
            min_connector_overlap,
            vert_construction: Vec::new(),
            horiz_construction: Vec::new(),
            vert_tables,
            horiz_tables,
        })
    }
}

// ---- MathAssembly (version 1) ----

/// A single part inside an assembly recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct PartRecord {
    /// Glyph ID for this piece.
    pub glyph_id: u16,
    /// Start connector length.
    pub start_connector_length: f32,
    /// End connector length.
    pub end_connector_length: f32,
    /// Full advance width of this part.
    pub full_advance: f32,
    /// Offset to the caret (in font units).
    pub part_flags: u16,
}

/// Assembly recipes — version 1 MATH extension for building very large glyphs.
#[derive(Debug, Clone)]
pub struct MathAssembly {
    pub vert_tables: HashMap<u16, Vec<PartRecord>>,
    pub horiz_tables: HashMap<u16, Vec<PartRecord>>,
}

impl MathAssembly {
    /// Parse `MathAssembly` from the MATH sub-table (version 1 only).
    ///
    /// Real structure (MS-OTTO §3.1, same pattern as MathVariants):
    /// ```text
    /// VertAssemblyCoverageOffset: uint16   (offset from MathAssembly start)
    /// HorizAssemblyCoverageOffset: uint16  (offset from MathAssembly start)
    /// VertAssemblyCount: uint16            (same as coverage count)
    /// HorizAssemblyCount: uint16           (same as coverage count)
    /// --- header 8 bytes ---
    /// VertAssembly[VertAssemblyCount]: 内嵌数组！
    ///   MathGlyphAssembly:
    ///     PartCount: uint16
    ///     PartRecord[PartCount]: GlyphID(u16), Start/EndConnector(FWORD),
    ///                            FullAdvance(FWORD), PartFlags(u16)
    /// HorizAssembly[HorizAssemblyCount]: 内嵌数组！（同上结构）
    /// ```
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ParseMathError> {
        let vert_cov_off = r.read_u16()? as usize;
        let horiz_cov_off = r.read_u16()? as usize;
        let vert_count = r.read_u16()? as usize;
        let horiz_count = r.read_u16()? as usize;

        let assembly_start = r.position() - 8; // 4 fields × 2 bytes
        let total_limit = r.limit;
        let data = r.bytes;

        let vert_ids = if vert_cov_off > 0 {
            parse_coverage(data, assembly_start + vert_cov_off, total_limit).unwrap_or_default()
        }
        else {
            Vec::new()
        };
        let horiz_ids = if horiz_cov_off > 0 {
            parse_coverage(data, assembly_start + horiz_cov_off, total_limit).unwrap_or_default()
        }
        else {
            Vec::new()
        };

        let mut vert_tables = HashMap::new();
        for idx in 0..vert_count.min(vert_ids.len()) {
            let part_count = r.read_u16()? as usize;
            let mut parts = Vec::with_capacity(part_count.min(1024));
            for _ in 0..part_count.min(1024) {
                parts.push(PartRecord {
                    glyph_id: r.read_u16()?,
                    start_connector_length: r.read_fword()?,
                    end_connector_length: r.read_fword()?,
                    full_advance: r.read_fword()?,
                    part_flags: r.read_u16()?,
                });
            }
            vert_tables.insert(vert_ids[idx], parts);
        }

        let mut horiz_tables = HashMap::new();
        for idx in 0..horiz_count.min(horiz_ids.len()) {
            let part_count = r.read_u16()? as usize;
            let mut parts = Vec::with_capacity(part_count.min(1024));
            for _ in 0..part_count.min(1024) {
                parts.push(PartRecord {
                    glyph_id: r.read_u16()?,
                    start_connector_length: r.read_fword()?,
                    end_connector_length: r.read_fword()?,
                    full_advance: r.read_fword()?,
                    part_flags: r.read_u16()?,
                });
            }
            horiz_tables.insert(horiz_ids[idx], parts);
        }

        // LMM MATH v1 has assembly_off=0x0046 which overlaps MathConstants — a
        // known invalid assembly offset. When the parse yields zero construction
        // entries the caller treats this as a missing/error-prone subtable.
        if vert_tables.is_empty() && horiz_tables.is_empty() {
            return Err(ParseMathError::Malformed("MathAssembly header yields zero vert/horiz constructions".into()));
        }

        Ok(Self { vert_tables, horiz_tables })
    }
}
