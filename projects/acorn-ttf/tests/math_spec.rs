//! `MathGlyphInfo` / `MathVariants` conformance and bounded-failure tests.
//!
//! Ground truth comes from two real fonts parsed field by field against
//! `ttf-parser`:
//! 1. Latin Modern Math (OTF/CFF, MATH v1) — the font VuTeX ships.
//! 2. Cambria Math (TrueType `glyf`, face 1 of `cambria.ttc`).
//!
//! The remaining tests build minimal `MATH` payloads by hand and check that
//! bad offsets, truncation, out-of-range glyph IDs and oversized structures
//! produce structured errors instead of panicking or looping.
//!
//! Font assertions are skipped when the font file is absent.

use acorn_ttf::{
    font::FontFace,
    sfnt::parse_font_validated,
    tables::{
        glyf::parse_glyph,
        math::{MathConstants, MathGlyphConstruction, MathGlyphInfo, MathKernTable, MathTable, ParseMathError},
    },
};
use std::collections::HashMap;
use ttf_parser::opentype_layout::Coverage;

/// Latin Modern Math shipped with the VuTeX workspace.
const LMM_PATH: &str = r#"E:\dxo 深度学习\VuTeX\projects\crates\vutex-text\assets\LatinModernMath-Regular.otf"#;
/// TrueType-flavoured math font (face 1 of the Windows Cambria collection).
const CAMBRIA_TTC_PATH: &str = r"C:\Windows\Fonts\cambria.ttc";

fn read_font(path: &str) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(data) => Some(data),
        Err(_) => {
            eprintln!("SKIP: font not available at {path}");
            None
        }
    }
}

fn acorn_math(data: &[u8], face_index: usize) -> MathTable {
    let container = parse_font_validated(data).expect("container");
    let face = container.face(face_index).expect("face index").clone();
    let mut font = FontFace::from_face(face);
    font.math().expect("MATH table").clone()
}

// ---- differential helpers against ttf-parser ----

fn diff_math_values(
    ours: &HashMap<u16, f32>,
    theirs: Option<ttf_parser::math::MathValues<'_>>,
    num_glyphs: u16,
    what: &str,
) {
    let mut expected = HashMap::new();
    if let Some(values) = theirs {
        for glyph_id in 0..num_glyphs {
            if let Some(value) = values.get(ttf_parser::GlyphId(glyph_id)) {
                expected.insert(glyph_id, value.value as f32);
            }
        }
    }
    assert_eq!(ours.len(), expected.len(), "{what}: glyph count differs from ttf-parser");
    for (glyph_id, value) in &expected {
        assert_eq!(ours.get(glyph_id), Some(value), "{what}: glyph {glyph_id} differs from ttf-parser");
    }
}

fn diff_extended_shapes(ours: &[u16], theirs: Option<Coverage<'_>>, num_glyphs: u16) {
    // `Coverage` only exposes lookups, so membership is checked per glyph.
    let mut expected = Vec::new();
    if let Some(coverage) = theirs {
        for glyph_id in 0..num_glyphs {
            if coverage.contains(ttf_parser::GlyphId(glyph_id)) {
                expected.push(glyph_id);
            }
        }
    }
    assert_eq!(ours, expected.as_slice(), "extended shapes differ from ttf-parser");
}

/// One of the four `MathKernInfoRecord` corners.
#[derive(Clone, Copy, Debug)]
enum Corner {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl Corner {
    fn ours(self, info: &MathGlyphInfo) -> &HashMap<u16, MathKernTable> {
        match self {
            Self::TopRight => &info.math_kern_top_right,
            Self::TopLeft => &info.math_kern_top_left,
            Self::BottomRight => &info.math_kern_bottom_right,
            Self::BottomLeft => &info.math_kern_bottom_left,
        }
    }

    fn theirs<'a>(self, info: &ttf_parser::math::KernInfo<'a>) -> Option<ttf_parser::math::Kern<'a>> {
        match self {
            Self::TopRight => info.top_right.clone(),
            Self::TopLeft => info.top_left.clone(),
            Self::BottomRight => info.bottom_right.clone(),
            Self::BottomLeft => info.bottom_left.clone(),
        }
    }
}

fn diff_kern_corner(
    ours: &MathGlyphInfo,
    theirs: Option<ttf_parser::math::KernInfos<'_>>,
    num_glyphs: u16,
    corner: Corner,
) {
    let what = format!("{corner:?} kern");
    let ours = corner.ours(ours);
    let mut seen = 0usize;
    if let Some(infos) = theirs {
        for glyph_id in 0..num_glyphs {
            let Some(info) = infos.get(ttf_parser::GlyphId(glyph_id))
            else {
                continue;
            };
            let Some(kern) = corner.theirs(&info)
            else {
                assert!(!ours.contains_key(&glyph_id), "{what}: glyph {glyph_id} has no kern in ttf-parser");
                continue;
            };
            seen += 1;
            let ours =
                ours.get(&glyph_id).unwrap_or_else(|| panic!("{what}: missing kern table for glyph {glyph_id}"));
            let count = kern.count();
            assert_eq!(ours.correction_heights.len() as u16, count, "{what}: glyph {glyph_id} height count");
            for index in 0..count {
                let expected = kern.height(index).expect("ttf-parser height").value as f32;
                assert_eq!(
                    ours.correction_heights[index as usize], expected,
                    "{what}: glyph {glyph_id} height {index} differs from ttf-parser"
                );
            }
            assert_eq!(ours.kern_values.len() as u16, count + 1, "{what}: glyph {glyph_id} kern count");
            for index in 0..=count {
                let expected = kern.kern(index).expect("ttf-parser kern").value as f32;
                assert_eq!(
                    ours.kern_values[index as usize], expected,
                    "{what}: glyph {glyph_id} kern {index} differs from ttf-parser"
                );
            }
        }
    }
    assert_eq!(ours.len(), seen, "{what}: kern glyph count differs from ttf-parser");
}

fn diff_constructions(
    ours: &HashMap<u16, MathGlyphConstruction>,
    coverages: &[u16],
    theirs: ttf_parser::math::GlyphConstructions<'_>,
    num_glyphs: u16,
    what: &str,
) {
    let mut seen = 0usize;
    for glyph_id in 0..num_glyphs {
        let Some(construction) = theirs.get(ttf_parser::GlyphId(glyph_id))
        else {
            continue;
        };
        seen += 1;
        let ours = ours.get(&glyph_id).unwrap_or_else(|| panic!("{what}: missing construction for glyph {glyph_id}"));

        let variant_count = construction.variants.len();
        assert_eq!(ours.variants.len() as u16, variant_count, "{what}: glyph {glyph_id} variant count");
        for index in 0..variant_count {
            let variant = construction.variants.get(index).expect("ttf-parser variant");
            let ours_variant = &ours.variants[index as usize];
            assert_eq!(
                ours_variant.variant_glyph, variant.variant_glyph.0,
                "{what}: glyph {glyph_id} variant {index} glyph differs from ttf-parser"
            );
            assert_eq!(
                ours_variant.advance_measurement, variant.advance_measurement as f32,
                "{what}: glyph {glyph_id} variant {index} advance differs from ttf-parser"
            );
        }

        match (&ours.assembly, construction.assembly) {
            (None, None) => {}
            (Some(ours_assembly), Some(assembly)) => {
                assert_eq!(
                    ours_assembly.italics_correction, assembly.italics_correction.value as f32,
                    "{what}: glyph {glyph_id} assembly italics correction differs from ttf-parser"
                );
                let part_count = assembly.parts.len();
                assert_eq!(
                    ours_assembly.parts.len() as u16, part_count,
                    "{what}: glyph {glyph_id} assembly part count"
                );
                for index in 0..part_count {
                    let part = assembly.parts.get(index).expect("ttf-parser part");
                    let ours_part = &ours_assembly.parts[index as usize];
                    assert_eq!(
                        ours_part.glyph_id, part.glyph_id.0,
                        "{what}: glyph {glyph_id} part {index} glyph differs from ttf-parser"
                    );
                    assert_eq!(
                        ours_part.start_connector_length, part.start_connector_length as f32,
                        "{what}: glyph {glyph_id} part {index} start connector differs from ttf-parser"
                    );
                    assert_eq!(
                        ours_part.end_connector_length, part.end_connector_length as f32,
                        "{what}: glyph {glyph_id} part {index} end connector differs from ttf-parser"
                    );
                    assert_eq!(
                        ours_part.full_advance, part.full_advance as f32,
                        "{what}: glyph {glyph_id} part {index} full advance differs from ttf-parser"
                    );
                    assert_eq!(
                        ours_part.part_flags, part.part_flags.0,
                        "{what}: glyph {glyph_id} part {index} flags differ from ttf-parser"
                    );
                }
            }
            _ => panic!("{what}: glyph {glyph_id} assembly presence differs from ttf-parser"),
        }
    }
    assert_eq!(ours.len(), seen, "{what}: construction count differs from ttf-parser");
    assert_eq!(coverages.len(), seen, "{what}: coverage count differs from ttf-parser");
}

fn diff_font(data: &[u8], face_index: u32) {
    let ours = acorn_math(data, face_index as usize);
    let tt_face = ttf_parser::Face::parse(data, face_index).expect("ttf-parser face");
    let num_glyphs = tt_face.number_of_glyphs();
    let theirs = tt_face.tables().math.expect("ttf-parser MATH");

    let glyph_info = theirs.glyph_info.expect("ttf-parser MathGlyphInfo");
    diff_math_values(&ours.glyph_info.italic_correction, glyph_info.italic_corrections, num_glyphs, "italic correction");
    diff_math_values(
        &ours.glyph_info.top_accent_attachment,
        glyph_info.top_accent_attachments,
        num_glyphs,
        "top accent attachment",
    );
    diff_extended_shapes(&ours.glyph_info.extended_shapes, glyph_info.extended_shapes, num_glyphs);
    for corner in [Corner::TopRight, Corner::TopLeft, Corner::BottomRight, Corner::BottomLeft] {
        diff_kern_corner(&ours.glyph_info, glyph_info.kern_infos, num_glyphs, corner);
    }

    let variants = theirs.variants.expect("ttf-parser MathVariants");
    assert_eq!(
        ours.variants.min_connector_overlap, variants.min_connector_overlap as f32,
        "MinConnectorOverlap differs from ttf-parser"
    );
    diff_constructions(
        &ours.variants.vert_constructions,
        &ours.variants.vert_coverages,
        variants.vertical_constructions,
        num_glyphs,
        "vertical construction",
    );
    diff_constructions(
        &ours.variants.horiz_constructions,
        &ours.variants.horiz_coverages,
        variants.horizontal_constructions,
        num_glyphs,
        "horizontal construction",
    );
}

#[test]
fn glyph_info_and_variants_match_ttf_parser_on_latin_modern_math() {
    let Some(data) = read_font(LMM_PATH)
    else {
        return;
    };
    diff_font(&data, 0);

    // Independent counts from fontTools 4.67.0 on the font whose SHA-256 is
    // 6075562B771F8B82F0C179E363389684F2DD09DE30038269E2628E504BD7BE0F.
    let math = acorn_math(&data, 0);
    let info = &math.glyph_info;
    assert_eq!(info.italic_correction.len(), 1002);
    assert_eq!(info.top_accent_attachment.len(), 2475);
    assert_eq!(info.extended_shapes.len(), 250);
    // Latin Modern Math ships no `MathKernInfo` subtable at all, so every
    // corner must stay empty even though the header offset parses.
    assert!(info.math_kern_top_right.is_empty());
    assert!(info.math_kern_top_left.is_empty());
    assert!(info.math_kern_bottom_right.is_empty());
    assert!(info.math_kern_bottom_left.is_empty());
    assert_eq!(math.variants.min_connector_overlap, 20.0);
    assert_eq!(math.variants.vert_coverages.len(), 94);
    assert_eq!(math.variants.horiz_coverages.len(), 86);
}

#[test]
fn glyph_info_and_variants_match_ttf_parser_on_cambria_math() {
    let Some(data) = read_font(CAMBRIA_TTC_PATH)
    else {
        return;
    };
    diff_font(&data, 1);

    // Independent values read from Cambria Math with fontTools.
    let math = acorn_math(&data, 1);
    let info = &math.glyph_info;
    assert_eq!(info.italic_correction.len(), 312);
    assert_eq!(info.italic_correction.get(&1325), Some(&50.0));
    assert_eq!(info.top_accent_attachment.len(), 439);
    assert_eq!(info.top_accent_attachment.get(&432), Some(&-364.0));
    assert_eq!(info.extended_shapes.len(), 227);
    assert_eq!(info.extended_shapes.first(), Some(&1516));

    // Cambria Math carries 438 MathKernInfo records but only 371 of them have a
    // top-right `MathKern` and 261 a bottom-right one. fontTools reports the
    // same split: top-right 371, bottom-right 261, top-left 39, bottom-left 124.
    assert_eq!(info.math_kern_top_right.len(), 371);
    assert_eq!(info.math_kern_top_left.len(), 39);
    assert_eq!(info.math_kern_bottom_right.len(), 261);
    assert_eq!(info.math_kern_bottom_left.len(), 124);
    let kern = info.math_kern_top_right.get(&4).expect("top-right kern for glyph 4");
    assert_eq!(kern.correction_heights, vec![620.0]);
    assert_eq!(kern.kern_values, vec![0.0, -75.0]);
    let bottom = info.math_kern_bottom_right.get(&4).expect("bottom-right kern for glyph 4");
    assert!(bottom.correction_heights.is_empty());
    assert_eq!(bottom.kern_values, vec![40.0]);
    // Left corners are populated by a different, smaller set of glyphs.
    let top_left = info.math_kern_top_left.get(&1827).expect("top-left kern for glyph 1827");
    assert_eq!(top_left.correction_heights, vec![616.0]);
    assert_eq!(top_left.kern_values, vec![0.0, -140.0]);
    let bottom_left = info.math_kern_bottom_left.get(&1775).expect("bottom-left kern for glyph 1775");
    assert_eq!(bottom_left.correction_heights, vec![420.0]);
    assert_eq!(bottom_left.kern_values, vec![65.0, 0.0]);

    let variants = &math.variants;
    assert_eq!(variants.min_connector_overlap, 200.0);
    assert_eq!(variants.vert_coverages.len(), 56);
    assert_eq!(variants.horiz_coverages.len(), 37);
    let first = variants.vert_constructions.get(&513).expect("vertical construction for glyph 513");
    assert_eq!(first.variants.len(), 8);
    assert_eq!(first.variants[0].variant_glyph, 513);
    assert_eq!(first.variants[0].advance_measurement, 1862.0);
    let assembly = first.assembly.as_ref().expect("glyph 513 assembly");
    assert_eq!(assembly.italics_correction, 0.0);
    assert_eq!(assembly.parts.len(), 2);
    assert_eq!(assembly.parts[0].glyph_id, 4720);
    assert_eq!(assembly.parts[0].start_connector_length, 8500.0);
    assert_eq!(assembly.parts[0].full_advance, 8756.0);
    assert_eq!(assembly.parts[0].part_flags, 1);
}

// ---- synthetic MATH payloads ----

/// A `MathConstants` record with distinguishable values for every field.
fn synthetic_constants_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MathConstants::BYTE_LEN);
    bytes.extend_from_slice(&700i16.to_be_bytes());
    bytes.extend_from_slice(&500i16.to_be_bytes());
    bytes.extend_from_slice(&1300u16.to_be_bytes());
    bytes.extend_from_slice(&1400u16.to_be_bytes());
    for i in 1..=51i16 {
        bytes.extend_from_slice(&(i * 10).to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
    }
    bytes.extend_from_slice(&60u16.to_be_bytes());
    bytes
}

fn u16s(values: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes
}

fn header(constants_off: u16, glyph_info_off: u16, variants_off: u16) -> Vec<u8> {
    let mut data = u16s(&[1, 0, constants_off, glyph_info_off, variants_off]);
    data.extend_from_slice(&synthetic_constants_bytes());
    data
}

fn math_with_glyph_info(glyph_info: &[u8]) -> Vec<u8> {
    let glyph_info_off = (10 + MathConstants::BYTE_LEN) as u16;
    let mut data = header(10, glyph_info_off, 0);
    data.extend_from_slice(glyph_info);
    data
}

fn math_with_variants(variants: &[u8]) -> Vec<u8> {
    let variants_off = (10 + MathConstants::BYTE_LEN) as u16;
    let mut data = header(10, 0, variants_off);
    data.extend_from_slice(variants);
    data
}

#[test]
fn absent_glyph_info_and_variants_default_to_empty() {
    let data = math_with_glyph_info(&u16s(&[0, 0, 0, 0]));
    let math = MathTable::parse(&data).expect("constants-only MATH parses");
    assert_eq!(math.version.0, 1);
    assert!(math.glyph_info.italic_correction.is_empty());
    assert!(math.glyph_info.extended_shapes.is_empty());
    assert!(math.glyph_info.math_kern_top_right.is_empty());
    assert!(math.variants.vert_coverages.is_empty());
    assert!(math.variants.vert_constructions.is_empty());
    assert_eq!(math.variants.min_connector_overlap, 0.0);
}

#[test]
fn bounded_rejects_unsupported_major_version() {
    let mut data = math_with_glyph_info(&u16s(&[0, 0, 0, 0]));
    data[0..2].copy_from_slice(&2u16.to_be_bytes());
    assert!(matches!(MathTable::parse(&data), Err(ParseMathError::UnsupportedVersion(2))));
}

#[test]
fn bounded_rejects_offsets_past_table_end() {
    // MathConstants is required, so a zero offset cannot be ignored.
    let mut data = math_with_glyph_info(&u16s(&[0, 0, 0, 0]));
    data[4..6].copy_from_slice(&0u16.to_be_bytes());
    assert!(matches!(MathTable::parse(&data), Err(ParseMathError::Malformed(_))));

    // MathGlyphInfo past the end of the payload.
    let mut data = math_with_glyph_info(&u16s(&[0, 0, 0, 0]));
    data[6..8].copy_from_slice(&0xFFFFu16.to_be_bytes());
    assert!(matches!(MathTable::parse(&data), Err(ParseMathError::Malformed(_))));

    // MathVariants past the end of the payload.
    let mut data = math_with_glyph_info(&u16s(&[0, 0, 0, 0]));
    data[8..10].copy_from_slice(&0xFFFFu16.to_be_bytes());
    assert!(matches!(MathTable::parse(&data), Err(ParseMathError::Malformed(_))));
}

#[test]
fn bounded_rejects_truncated_payloads() {
    let data = math_with_glyph_info(&u16s(&[0, 0, 0, 0]));
    // Header plus a partial MathConstants record.
    assert!(matches!(MathTable::parse(&data[..120]), Err(ParseMathError::Bounds(_))));
    // Header only: the constants offset points at byte 10, which no longer exists.
    assert!(matches!(MathTable::parse(&data[..10]), Err(ParseMathError::Bounds(_))));
}

#[test]
fn bounded_rejects_unknown_coverage_format() {
    // MathGlyphInfo -> MathItalicsCorrectionInfo whose coverage claims format 3.
    let glyph_info = u16s(&[
        8, // MathItalicsCorrectionInfo at +8
        0, // no top accent attachment
        0, // no extended shapes
        0, // no MathKernInfo
        8, // coverage at +8 from the italics subtable
        1, // ItalicsCorrectionCount
        5, // MathValueRecord value
        0, // deviceTableOffset
        3, // Coverage format 3 (invalid)
        1, // coverage glyph count
    ]);
    let data = math_with_glyph_info(&glyph_info);
    assert!(matches!(MathTable::parse(&data), Err(ParseMathError::Malformed(_))));
}

#[test]
fn bounded_rejects_glyph_ids_outside_maxp() {
    let glyph_info = u16s(&[
        8, // MathItalicsCorrectionInfo at +8
        0,
        0,
        0,
        8, // coverage at +8
        1, // count
        5, // value
        0, // device offset
        1, // Coverage format 1
        1, // one glyph
        5000, // glyph id far beyond numGlyphs 10
    ]);
    let data = math_with_glyph_info(&glyph_info);
    match MathTable::parse_with_num_glyphs(&data, Some(10)) {
        Err(ParseMathError::GlyphIdOutOfRange { glyph_id: 5000, num_glyphs: 10, what: "MathItalicsCorrectionInfo" }) => {}
        other => panic!("unexpected result: {other:?}"),
    }
    // Without a glyph count the same table parses, which is why `FontFace` supplies maxp.
    assert!(MathTable::parse(&data).is_ok());
}

#[test]
fn bounded_rejects_oversized_coverage_range() {
    // Two Coverage format 2 ranges of 0..=65535 would expand to 131072 glyphs.
    let glyph_info = u16s(&[
        8, // MathItalicsCorrectionInfo at +8
        0,
        0,
        0,
        4, // coverage at +4
        0, // count
        2, // Coverage format 2
        2, // two ranges
        0, 65535, 0, // first full range
        0, 65535, 0, // second full range
    ]);
    let data = math_with_glyph_info(&glyph_info);
    match MathTable::parse(&data) {
        Err(ParseMathError::LimitExceeded { what: "coverage glyphs", count: 131_072, max: 65_536 }) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn bounded_rejects_oversized_variant_count() {
    let variants = u16s(&[
        20, // MinConnectorOverlap
        30, // VertGlyphCoverage at +30
        0, // no horizontal coverage
        1, // VertGlyphCount
        0, // HorizGlyphCount
        16, // construction at +16
        0,
        0, // padding up to +16
        0, // GlyphAssemblyOffset
        65000, // VariantCount above the cap
        0, 0, 0, 0, 0, // padding up to +30
        1, // Coverage format 1
        1, // one glyph
        3, // glyph id
    ]);
    let data = math_with_variants(&variants);
    match MathTable::parse(&data) {
        Err(ParseMathError::LimitExceeded { what: "MathGlyphConstruction variants", count: 65_000, max: 4096 }) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn bounded_rejects_oversized_assembly() {
    let variants = u16s(&[
        20, // MinConnectorOverlap
        30, // VertGlyphCoverage at +30
        0, // no horizontal coverage
        1, // VertGlyphCount
        0, // HorizGlyphCount
        16, // construction at +16
        0,
        0, // padding up to +16
        8, // GlyphAssembly at +8 from the construction
        0, // VariantCount
        0,
        0, // padding up to +24
        0, // assembly italics correction
        0, // device offset
        60000, // PartCount above the cap
        1, // Coverage format 1
        1, // one glyph
        3, // glyph id
    ]);
    let data = math_with_variants(&variants);
    match MathTable::parse(&data) {
        Err(ParseMathError::LimitExceeded { what: "assembly parts", count: 60_000, max: 1024 }) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn bounded_rejects_truncated_kern_heights() {
    // One `MathKernInfo` record whose top-right corner points at a `MathKern`
    // claiming four correction heights but only carrying one.
    let glyph_info = glyph_info_with_kern_corner(32);
    let mut bytes = u16s(&[4u16]); // HeightCount
    bytes.extend_from_slice(&500i16.to_be_bytes()); // CorrectionHeight[0]
    bytes.extend_from_slice(&0u16.to_be_bytes()); // its device offset
    let data = math_with_glyph_info(&glyph_info);
    let mut payload = data;
    payload.extend_from_slice(&bytes);
    assert!(matches!(MathTable::parse(&payload), Err(ParseMathError::Bounds(_))));
}

#[test]
fn bounded_rejects_oversized_kern_height_count() {
    let glyph_info = glyph_info_with_kern_corner(32);
    let data = math_with_glyph_info(&glyph_info);
    let mut payload = data;
    payload.extend_from_slice(&2000u16.to_be_bytes()); // HeightCount above the cap
    match MathTable::parse(&payload) {
        Err(ParseMathError::LimitExceeded { what: "kern heights", count: 2000, max: 1024 }) => {}
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn bounded_rejects_out_of_range_left_kern_offset() {
    // The two left corners are parsed and validated, not silently ignored.
    for corner in [1usize, 3] {
        let mut glyph_info = glyph_info_with_kern_corner(0);
        let mut list = Vec::with_capacity(glyph_info.len() / 2);
        for chunk in glyph_info.chunks_exact(2) {
            list.push(u16::from_be_bytes([chunk[0], chunk[1]]));
        }
        // Records start at u16 index 6: top-right, top-left, bottom-right,
        // bottom-left.
        list[6 + corner] = 0xFFFF;
        glyph_info.clear();
        glyph_info.extend_from_slice(&u16s(&list));
        let data = math_with_glyph_info(&glyph_info);
        assert!(
            matches!(MathTable::parse(&data), Err(ParseMathError::Malformed(_))),
            "corner {corner} must reject an offset past the table"
        );
    }
}

/// A `MathGlyphInfo` with one `MathKernInfo` record whose top-right corner is
/// at `top_right` (relative to `MathKernInfo`) and whose coverage names glyph 3.
///
/// Records live at u16 indices 6..10, so the shared layout is fixed: the caller
/// appends any kern subtable bytes right after this payload.
fn glyph_info_with_kern_corner(top_right: u16) -> Vec<u8> {
    u16s(&[
        0, 0, 0, // no italics, top accent or extended shapes
        8, // MathKernInfo at +8
        20, // MathKernCoverage at +20 from MathKernInfo
        1, // MathKernCount
        top_right, // TopRightMathKern
        0,         // TopLeftMathKern
        0,         // BottomRightMathKern
        0,         // BottomLeftMathKern
        0, 0, 0, 0, // padding up to +28
        1, // Coverage format 1
        1, // one glyph
        3, // glyph id 3
        0, 0, 0, // padding up to +40
    ])
}

#[test]
fn composite_glyph_cycle_does_not_recurse() {
    // A composite glyph whose only component is the glyph itself. Component
    // records are collected iteratively, so a cycle is reported as data rather
    // than expanded forever.
    const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
    const ARGS_ARE_XY_VALUES: u16 = 0x0002;

    let mut glyph = Vec::new();
    glyph.extend_from_slice(&(-1i16).to_be_bytes()); // numContours < 0 → composite
    for _ in 0..4 {
        glyph.extend_from_slice(&0i16.to_be_bytes()); // bbox
    }
    glyph.extend_from_slice(&(ARG_1_AND_2_ARE_WORDS | ARGS_ARE_XY_VALUES).to_be_bytes());
    glyph.extend_from_slice(&7u16.to_be_bytes()); // component glyph 7 == this glyph
    glyph.extend_from_slice(&0i16.to_be_bytes()); // x offset
    glyph.extend_from_slice(&0i16.to_be_bytes()); // y offset

    let outline = parse_glyph(7, &glyph).expect("composite parses");
    assert!(outline.is_composite);
    assert_eq!(outline.components.len(), 1);
    assert_eq!(outline.components[0].glyph_id, 7);
    assert!(outline.points.is_empty(), "composite glyphs carry no points of their own");
}
