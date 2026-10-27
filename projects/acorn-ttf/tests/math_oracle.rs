//! Glyph-level `MATH` expectations taken from an independent parser.
//!
//! Every number below was dumped with **fontTools 4.67.0** (`MATH` →
//! `MathGlyphInfo` / `MathVariants`, outline extents via `BoundsPen`), never
//! with `acorn-ttf` itself. That keeps this suite a real oracle: if our
//! `Coverage`, `MathKernInfo`, `MathGlyphConstruction` or `GlyphAssembly`
//! parsing drifts by a field or an offset, these assertions fail even when our
//! own round-trip helpers stay self-consistent.
//!
//! Fixtures are read-only and the assertions are skipped when a font is absent:
//! 1. `LatinModernMath-Regular.otf` — SHA-256
//!    `6075562B771F8B82F0C179E363389684F2DD09DE30038269E2628E504BD7BE0F`,
//!    4802 glyphs (the font VuTeX ships).
//! 2. `cambria.ttc` face 1 — SHA-256
//!    `84E70CCC1664482F4A960442C7A166C91A1B2CF98FF88C33CB73F79403F66D7B`,
//!    7614 glyphs.
//!
//! `height` and `depth` are `yMax` and `-yMin` of the largest prebuilt variant,
//! in font units. For vertical constructions `advance = height + depth` to
//! within one font unit, so the pair also cross-checks the variant advance
//! against independently measured outline extents.

use acorn_ttf::{
    font::FontFace,
    sfnt::parse_font_validated,
    tables::math::{MathKernTable, MathTable},
};

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

fn acorn_math(data: &[u8], face_index: usize) -> (MathTable, u16) {
    let container = parse_font_validated(data).expect("container");
    let face = container.face(face_index).expect("face index").clone();
    let mut font = FontFace::from_face(face);
    let num_glyphs = font.maxp().expect("maxp").num_glyphs;
    let math = font.math().expect("MATH table").clone();
    (math, num_glyphs)
}

/// Per-glyph `MathItalicsCorrectionInfo` and `MathTopAccentAttachment` values.
struct GlyphMetrics {
    character: char,
    gid: u16,
    italic_correction: Option<f32>,
    top_accent_attachment: Option<f32>,
}

/// Per-glyph vertical construction summary.
struct VerticalConstruction {
    character: char,
    gid: u16,
    variants: usize,
    max_variant_gid: u16,
    max_advance: f32,
    height: f32,
    depth: f32,
}

/// Per-glyph `GlyphAssembly` summary: italics correction and part records
/// `(glyph_id, start_connector, end_connector, full_advance, part_flags)`.
struct AssemblyExpectation {
    character: char,
    gid: u16,
    italics_correction: f32,
    parts: &'static [(u16, f32, f32, f32, u16)],
}

const LMM_METRICS: &[GlyphMetrics] = &[
    GlyphMetrics { character: 'f', gid: 71, italic_correction: Some(79.0), top_accent_attachment: Some(262.0) },
    GlyphMetrics { character: 'x', gid: 89, italic_correction: Some(16.0), top_accent_attachment: Some(258.0) },
    GlyphMetrics { character: 'A', gid: 34, italic_correction: None, top_accent_attachment: Some(375.0) },
    GlyphMetrics { character: 'V', gid: 55, italic_correction: Some(8.0), top_accent_attachment: Some(375.0) },
    GlyphMetrics { character: '(', gid: 9, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: '{', gid: 92, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: '∑', gid: 3060, italic_correction: None, top_accent_attachment: None },
];

const LMM_VERTICAL: &[VerticalConstruction] = &[
    VerticalConstruction {
        character: '(',
        gid: 9,
        variants: 8,
        max_variant_gid: 2499,
        max_advance: 2991.0,
        height: 1745.0,
        depth: 1245.0,
    },
    VerticalConstruction {
        character: '{',
        gid: 92,
        variants: 8,
        max_variant_gid: 2515,
        max_advance: 3001.0,
        height: 1750.0,
        depth: 1250.0,
    },
    VerticalConstruction {
        character: '∑',
        gid: 3060,
        variants: 2,
        max_variant_gid: 3074,
        max_advance: 1401.0,
        height: 950.0,
        depth: 450.0,
    },
];

const LMM_ASSEMBLIES: &[AssemblyExpectation] = &[
    AssemblyExpectation {
        character: '(',
        gid: 9,
        italics_correction: 0.0,
        parts: &[(2503, 0.0, 249.0, 1495.0, 0), (2504, 498.0, 498.0, 498.0, 1), (2505, 249.0, 0.0, 1495.0, 0)],
    },
    AssemblyExpectation {
        character: '{',
        gid: 92,
        italics_correction: 0.0,
        parts: &[
            (2517, 0.0, 374.0, 750.0, 0),
            (2518, 748.0, 748.0, 748.0, 1),
            (2519, 374.0, 374.0, 1500.0, 0),
            (2518, 748.0, 748.0, 748.0, 1),
            (2520, 374.0, 0.0, 750.0, 0),
        ],
    },
];

const CAMBRIA_METRICS: &[GlyphMetrics] = &[
    GlyphMetrics { character: 'f', gid: 136, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: 'x', gid: 154, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: 'A', gid: 4, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: 'V', gid: 25, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: '(', gid: 4666, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: '{', gid: 4668, italic_correction: None, top_accent_attachment: None },
    GlyphMetrics { character: '∑', gid: 963, italic_correction: None, top_accent_attachment: None },
];

const CAMBRIA_VERTICAL: &[VerticalConstruction] = &[
    VerticalConstruction {
        character: '(',
        gid: 4666,
        variants: 8,
        max_variant_gid: 3438,
        max_advance: 8881.0,
        height: 5025.0,
        depth: 3855.0,
    },
    VerticalConstruction {
        character: '{',
        gid: 4668,
        variants: 8,
        max_variant_gid: 3422,
        max_advance: 8801.0,
        height: 4985.0,
        depth: 3815.0,
    },
    VerticalConstruction {
        character: '∑',
        gid: 963,
        variants: 5,
        max_variant_gid: 3535,
        max_advance: 8590.0,
        height: 4880.0,
        depth: 3709.0,
    },
];

const CAMBRIA_ASSEMBLIES: &[AssemblyExpectation] = &[
    AssemblyExpectation {
        character: '(',
        gid: 4666,
        italics_correction: 0.0,
        parts: &[
            (1737, 0.0, 300.0, 4733.0, 0),
            (1736, 2500.0, 2500.0, 2501.0, 1),
            (1735, 300.0, 0.0, 4733.0, 0),
        ],
    },
    AssemblyExpectation {
        character: '{',
        gid: 4668,
        italics_correction: 0.0,
        parts: &[
            (1749, 0.0, 1000.0, 2546.0, 0),
            (1750, 2500.0, 2500.0, 2501.0, 1),
            (1748, 1000.0, 1000.0, 4688.0, 0),
            (1750, 2500.0, 2500.0, 2501.0, 1),
            (1747, 1000.0, 0.0, 2546.0, 0),
        ],
    },
];

/// `(glyph_id, corner height, kern values)` for Cambria's four corners.
/// The sample glyphs are the ones fontTools reports as populated for each
/// corner: 136 (`f`), 154 (`x`), 4 (`A`), 25 (`V`), 1827, 1775.
const CAMBRIA_KERNS: &[(u16, u16, &str, &[f32], &[f32])] = &[
    (136, 1, "top_right", &[680.0], &[-30.0, 250.0]),
    (154, 1, "top_right", &[660.0], &[0.0, 80.0]),
    (4, 1, "top_right", &[620.0], &[0.0, -75.0]),
    (25, 1, "top_right", &[924.0], &[-40.0, 80.0]),
    (154, 2, "bottom_right", &[], &[40.0]),
    (4, 2, "bottom_right", &[], &[40.0]),
    (25, 2, "bottom_right", &[800.0], &[-180.0, 0.0]),
    (1827, 0, "top_left", &[616.0], &[0.0, -140.0]),
    (1775, 3, "bottom_left", &[420.0], &[65.0, 0.0]),
];

fn corner<'a>(math: &'a MathTable, corner: u16) -> &'a std::collections::HashMap<u16, MathKernTable> {
    match corner {
        0 => &math.glyph_info.math_kern_top_left,
        1 => &math.glyph_info.math_kern_top_right,
        2 => &math.glyph_info.math_kern_bottom_right,
        3 => &math.glyph_info.math_kern_bottom_left,
        _ => panic!("unknown corner {corner}"),
    }
}

fn check_metrics(math: &MathTable, expected: &[GlyphMetrics]) {
    for glyph in expected {
        assert_eq!(
            math.glyph_info.italic_correction.get(&glyph.gid).copied(),
            glyph.italic_correction,
            "italic correction for {} (glyph {})",
            glyph.character,
            glyph.gid
        );
        assert_eq!(
            math.glyph_info.top_accent_attachment.get(&glyph.gid).copied(),
            glyph.top_accent_attachment,
            "top accent attachment for {} (glyph {})",
            glyph.character,
            glyph.gid
        );
    }
}

fn check_vertical(math: &MathTable, expected: &[VerticalConstruction]) {
    for construction in expected {
        let ours = math
            .variants
            .vert_constructions
            .get(&construction.gid)
            .unwrap_or_else(|| panic!("no vertical construction for {} (glyph {})", construction.character, construction.gid));
        assert!(
            math.variants.vert_coverages.contains(&construction.gid),
            "{} (glyph {}) is missing from the vertical coverage",
            construction.character,
            construction.gid
        );
        assert_eq!(
            ours.variants.len(),
            construction.variants,
            "vertical variant count for {}",
            construction.character
        );
        let largest = ours.variants.last().expect("at least one variant");
        assert_eq!(
            largest.variant_glyph, construction.max_variant_gid,
            "largest variant glyph for {}",
            construction.character
        );
        assert_eq!(
            largest.advance_measurement, construction.max_advance,
            "largest variant advance for {}",
            construction.character
        );
        // An independent outline measurement of the same glyph: the vertical
        // advance has to cover height plus depth.
        let advance = largest.advance_measurement;
        let extents = construction.height + construction.depth;
        assert!(
            (advance - extents).abs() <= 2.0,
            "{} vertical advance {advance} does not match height {} + depth {}",
            construction.character,
            construction.height,
            construction.depth
        );
        assert!(
            !math.variants.horiz_constructions.contains_key(&construction.gid),
            "{} unexpectedly has a horizontal construction",
            construction.character
        );
    }
}

fn check_assemblies(math: &MathTable, expected: &[AssemblyExpectation]) {
    for assembly in expected {
        let construction = math
            .variants
            .vert_constructions
            .get(&assembly.gid)
            .unwrap_or_else(|| panic!("no vertical construction for {} (glyph {})", assembly.character, assembly.gid));
        let ours = construction
            .assembly
            .as_ref()
            .unwrap_or_else(|| panic!("no assembly for {} (glyph {})", assembly.character, assembly.gid));
        assert_eq!(
            ours.italics_correction, assembly.italics_correction,
            "assembly italics correction for {}",
            assembly.character
        );
        assert_eq!(ours.parts.len(), assembly.parts.len(), "assembly part count for {}", assembly.character);
        for (index, (expected_part, ours_part)) in assembly.parts.iter().zip(&ours.parts).enumerate() {
            assert_eq!(
                (
                    ours_part.glyph_id,
                    ours_part.start_connector_length,
                    ours_part.end_connector_length,
                    ours_part.full_advance,
                    ours_part.part_flags
                ),
                *expected_part,
                "assembly part {index} for {}",
                assembly.character
            );
        }
    }
}

#[test]
fn latin_modern_math_matches_fonttools_oracle() {
    let Some(data) = read_font(LMM_PATH)
    else {
        return;
    };
    let (math, num_glyphs) = acorn_math(&data, 0);
    assert_eq!(num_glyphs, 4802, "fixture identity: Latin Modern Math glyph count");

    check_metrics(&math, LMM_METRICS);
    check_vertical(&math, LMM_VERTICAL);
    check_assemblies(&math, LMM_ASSEMBLIES);

    // fontTools counts on the same file.
    assert_eq!(math.glyph_info.italic_correction.len(), 1002);
    assert_eq!(math.glyph_info.top_accent_attachment.len(), 2475);
    assert_eq!(math.glyph_info.extended_shapes.len(), 250);
    assert_eq!(math.variants.vert_coverages.len(), 94);
    assert_eq!(math.variants.horiz_coverages.len(), 86);
    assert_eq!(math.variants.min_connector_overlap, 20.0);
}

#[test]
fn cambria_math_matches_fonttools_oracle() {
    let Some(data) = read_font(CAMBRIA_TTC_PATH)
    else {
        return;
    };
    let (math, num_glyphs) = acorn_math(&data, 1);
    assert_eq!(num_glyphs, 7614, "fixture identity: Cambria Math glyph count");

    check_metrics(&math, CAMBRIA_METRICS);
    check_vertical(&math, CAMBRIA_VERTICAL);
    check_assemblies(&math, CAMBRIA_ASSEMBLIES);

    for (gid, corner_id, name, heights, kern_values) in CAMBRIA_KERNS {
        let table = corner(&math, *corner_id)
            .get(gid)
            .unwrap_or_else(|| panic!("no {name} kern for glyph {gid}"));
        assert_eq!(table.correction_heights.as_slice(), *heights, "{name} heights for glyph {gid}");
        assert_eq!(table.kern_values.as_slice(), *kern_values, "{name} kerns for glyph {gid}");
    }

    // fontTools corner counts on the same file: 438 `MathKernInfo` records,
    // split 371 / 39 / 261 / 124 across the four corners.
    assert_eq!(math.glyph_info.math_kern_top_right.len(), 371);
    assert_eq!(math.glyph_info.math_kern_top_left.len(), 39);
    assert_eq!(math.glyph_info.math_kern_bottom_right.len(), 261);
    assert_eq!(math.glyph_info.math_kern_bottom_left.len(), 124);
    assert_eq!(math.glyph_info.italic_correction.len(), 312);
    assert_eq!(math.glyph_info.top_accent_attachment.len(), 439);
    assert_eq!(math.glyph_info.extended_shapes.len(), 227);
    assert_eq!(math.variants.vert_coverages.len(), 56);
    assert_eq!(math.variants.horiz_coverages.len(), 37);
    assert_eq!(math.variants.min_connector_overlap, 200.0);
}
