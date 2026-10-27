//! Smoke test that loads LatinModernMath-Regular.otf and reports facts.
//!
//! This is deliberately an integration test under `tests/` so that it runs
//! against the full public API of `acorn-ttf`. The font is loaded via an
//! absolute path — if the file moves, adjust `LMM_PATH` below.

use acorn_ttf::{font::FontFace, license::LicenseKind, sfnt, tables::PartRecord};

const LMM_PATH: &str = r#"E:\dxo 深度学习\VuTeX\projects\crates\vutex-text\assets\LatinModernMath-Regular.otf"#;

fn load_lmm() -> FontFace {
    let data = std::fs::read(LMM_PATH).expect("LMM OTF must be present at LMM_PATH");
    FontFace::load(&data).expect("parse LMM OTF")
}

#[test]
fn smoke_lmm_container() {
    let mut face = load_lmm();
    assert!(face.is_opentype_cff());
    assert!(!face.is_truetype_glyf());
    assert!(face.has_math());

    // LatinModernMath has 14 SFNT tables (head hhea maxp OS/2 name cmap post
    // hmtx loca glyf CFF VORG GPOS GSUB MATH BASE+DSIG).
    assert!(face.face().directory.len() >= 10);

    let family = face.family().expect("family");
    assert!(family.contains("Latin"), "family={family}");
    let full = face.full_name().expect("full name");
    assert!(full.contains("Math"), "full={full}");
}

#[test]
fn smoke_lmm_head_maxp() {
    let mut face = load_lmm();
    let head = face.head().expect("head");
    assert_eq!(head.magic_number, 0x5F0F_3CF5);
    assert_eq!(head.units_per_em, 1000);

    let maxp = face.maxp().expect("maxp");
    assert_eq!(maxp.num_glyphs, 4802, "LMM maxp version 0.5 has exactly 4802 glyphs");
}

#[test]
fn smoke_lmm_cmap() {
    let mut face = load_lmm();
    let cmap = face.cmap().expect("cmap");
    assert!(cmap.lookup('+' as u32).is_some());
    assert!(cmap.lookup('=' as u32).is_some());
    assert!(cmap.lookup('∫' as u32).is_some());
    assert!(cmap.lookup('∈' as u32).is_some());
    // LMM does not carry U+1D400 math bold cap-A — it is a math operator set
    // not a full styled Latin glyph set, so we intentionally omit that check.
}

#[test]
fn smoke_lmm_math_constants() {
    let mut face = load_lmm();
    let math = face.math().expect("MATH");
    assert_eq!(math.version.0, 1, "LMM uses MATH version 1");
    // sanity check a few constants in expected ballpark.
    let c = &math.constants;
    assert!((c.axis_height - 250.0).abs() < 10.0, "axis_height={}", c.axis_height);
    // LMM stores ScriptPercentScaleDown as an integer percentage (Short = 70
    // and 50), not as a 0–1 fraction. We assert the raw parsed value.
    assert!((c.script_percent_scale_down - 70.0).abs() < 1.0, "script_percent={}", c.script_percent_scale_down);
    assert!(
        (c.script_script_percent_scale_down - 50.0).abs() < 1.0,
        "scriptscript_percent={}",
        c.script_script_percent_scale_down
    );
}

#[test]
fn smoke_lmm_math_glyph_info() {
    let mut face = load_lmm();
    let math = face.math().expect("MATH");
    let info = &math.glyph_info;
    // Counts and values below come from fontTools on LatinModernMath-Regular.otf.
    assert_eq!(info.italic_correction.len(), 1002, "LMM italic correction count");
    assert_eq!(info.italic_correction.get(&24), Some(&13.0), "glyph 24 italic correction");
    assert_eq!(info.top_accent_attachment.len(), 2475, "LMM top accent count");
    assert_eq!(info.top_accent_attachment.get(&11), Some(&398.0), "glyph 11 accent attachment");
    assert_eq!(info.extended_shapes.len(), 250, "LMM extended shape count");
    assert_eq!(info.extended_shapes.first(), Some(&1869), "first extended shape glyph");
    // Latin Modern Math carries no MathKernInfo subtable.
    assert!(info.math_kern_top_right.is_empty());
    assert!(info.math_kern_bottom_right.is_empty());
}

#[test]
fn smoke_lmm_math_variants() {
    let mut face = load_lmm();
    let math = face.math().expect("MATH");
    let variants = &math.variants;
    assert_eq!(variants.min_connector_overlap, 20.0);
    assert_eq!(variants.vert_coverages.len(), 94);
    assert_eq!(variants.horiz_coverages.len(), 86);
    assert_eq!(variants.vert_constructions.len(), 94);
    assert_eq!(variants.horiz_constructions.len(), 86);

    // Glyph 9 (`parenleft`) carries the canonical eight-step ladder plus a
    // three-part assembly whose middle part is the extender.
    let paren = variants.vert_constructions.get(&9).expect("vertical construction for glyph 9");
    assert_eq!(paren.glyph_id, 9);
    assert_eq!(paren.variants.len(), 8);
    assert_eq!(paren.variants[0].variant_glyph, 9);
    assert_eq!(paren.variants[0].advance_measurement, 997.0);
    assert_eq!(paren.variants[7].advance_measurement, 2991.0);

    let assembly = paren.assembly.as_ref().expect("glyph 9 assembly");
    assert_eq!(assembly.italics_correction, 0.0);
    assert_eq!(assembly.parts.len(), 3);
    assert_eq!(assembly.parts[1].glyph_id, 2504);
    assert_eq!(assembly.parts[1].start_connector_length, 498.0);
    assert_eq!(assembly.parts[1].full_advance, 498.0);
    assert_eq!(assembly.parts[1].part_flags & PartRecord::EXTENDER, PartRecord::EXTENDER);
}

#[test]
fn smoke_lmm_license() {
    let mut face = load_lmm();
    let info = acorn_ttf::license::LicenseInfo::from_face(&mut face);
    // LMM is under GUST Font License / SIL OFL.
    assert!(matches!(info.kind, LicenseKind::Gust | LicenseKind::OfL), "kind={:?}", info.kind);
    assert!(info.embedding_permitted);
}

#[test]
fn smoke_unknown_version_rejected() {
    let bytes = [0x7F, 0x00, 0x00, 0x00, 0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(sfnt::parse_font(&bytes).is_err());
}
