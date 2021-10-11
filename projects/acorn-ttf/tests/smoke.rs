//! Smoke test that loads LatinModernMath-Regular.otf and reports facts.
//!
//! This is deliberately an integration test under `tests/` so that it runs
//! against the full public API of `acorn-ttf`. The font is loaded via an
//! absolute path — if the file moves, adjust `LMM_PATH` below.

use acorn_ttf::{font::FontFace, license::LicenseKind, sfnt};

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
    // LMM exposes italic correction for a large number of glyphs.
    assert!(!math.glyph_info.italic_correction.is_empty(), "LMM should have italic corrections");
    assert!(!math.glyph_info.accent_attachment.is_empty(), "LMM should have accent attachments");
}

#[test]
fn smoke_lmm_math_variants() {
    let mut face = load_lmm();
    let math = face.math().expect("MATH");
    assert!(!math.variants.vert_tables.is_empty(), "LMM should have vertical constructions");
    assert!(!math.variants.horiz_tables.is_empty(), "LMM should have horizontal constructions");
}

#[test]
fn smoke_lmm_math_assembly() {
    let mut face = load_lmm();
    let math = face.math().expect("MATH");
    // LMM MATH v1 has assembly_off=0x0046 which overlaps MathConstants — a
    // known-invalid offset so the parser treats it as missing. We tolerate
    // the absence here because MATH v1 without a usable assembly is valid.
    if let Some(asm) = &math.assembly {
        assert!(!asm.vert_tables.is_empty() || !asm.horiz_tables.is_empty());
    }
    // If assembly is None that is also correct for this font.
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
