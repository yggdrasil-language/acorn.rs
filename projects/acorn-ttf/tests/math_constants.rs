//! `MathConstants` conformance against MS-OTTO §3.1.
//!
//! Three independent sources are used so the parser cannot self-certify:
//! 1. a synthetic 214-byte record with known sequential values,
//! 2. Latin Modern Math (OTF/CFF, MATH v1) cross-checked field by field
//!    against `ttf-parser`,
//! 3. Cambria Math (TrueType `glyf` inside `cambria.ttc`, face 1) cross-checked
//!    the same way.
//!
//! Font assertions are skipped when the font file is absent, so the suite
//! still runs on machines without those specific fonts.

use acorn_ttf::{
    font::FontFace,
    reader::Reader,
    sfnt::parse_font_validated,
    tables::math::{MathConstants, MathTable},
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

fn acorn_math(data: &[u8], face_index: usize) -> MathTable {
    let container = parse_font_validated(data).expect("container");
    let face = container.face(face_index).expect("face index").clone();
    let mut font = FontFace::from_face(face);
    let math = font.math().expect("MATH table").clone();
    math
}

fn tt_constants(data: &[u8], face_index: u32) -> ttf_parser::math::Constants<'_> {
    let face = ttf_parser::Face::parse(data, face_index).expect("ttf-parser face");
    face.tables().math.expect("ttf-parser MATH").constants.expect("ttf-parser MathConstants")
}

/// A `MathConstants` record with distinguishable values for every field.
fn synthetic_constants() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MathConstants::BYTE_LEN);
    bytes.extend_from_slice(&700i16.to_be_bytes()); // ScriptPercentScaleDown
    bytes.extend_from_slice(&500i16.to_be_bytes()); // ScriptScriptPercentScaleDown
    bytes.extend_from_slice(&1300u16.to_be_bytes()); // DelimitedSubFormulaMinHeight
    bytes.extend_from_slice(&1400u16.to_be_bytes()); // DisplayOperatorMinHeight
    for i in 1..=51i16 {
        bytes.extend_from_slice(&(i * 10).to_be_bytes()); // MathValueRecord value
        bytes.extend_from_slice(&0u16.to_be_bytes()); // deviceTableOffset
    }
    bytes.extend_from_slice(&60u16.to_be_bytes()); // RadicalDegreeBottomRaisePercent
    bytes
}

#[test]
fn constants_layout_follows_specification_order() {
    let bytes = synthetic_constants();
    assert_eq!(bytes.len(), MathConstants::BYTE_LEN);

    let mut reader = Reader::new(&bytes);
    let c = MathConstants::parse(&mut reader).expect("parse synthetic constants");
    assert_eq!(reader.position(), MathConstants::BYTE_LEN, "exactly 214 bytes are consumed");

    // 4 simple fields
    assert_eq!(c.script_percent_scale_down, 700.0);
    assert_eq!(c.script_script_percent_scale_down, 500.0);
    assert_eq!(c.delimited_sub_formula_min_height, 1300.0);
    assert_eq!(c.display_operator_min_height, 1400.0);

    // MathValueRecord #n (1-based) carries the value n * 10.
    assert_eq!(c.math_leading, 10.0, "record 1");
    assert_eq!(c.axis_height, 20.0, "record 2");
    assert_eq!(c.superscript_baseline_drop_max, 110.0, "record 11");
    assert_eq!(c.space_after_script, 140.0, "record 14: SpaceAfterScript follows SuperscriptBottomMaxWithSubscript");
    assert_eq!(c.stack_gap_min, 230.0, "record 23");
    assert_eq!(c.fraction_rule_thickness, 350.0, "record 35");
    assert_eq!(c.radical_vertical_gap, 460.0, "record 46");
    assert_eq!(c.radical_kern_after_degree, 510.0, "record 51");
    assert_eq!(c.radical_degree_bottom_raise_percent, 60.0);
}

#[test]
fn constants_reject_truncated_record() {
    let mut bytes = synthetic_constants();
    bytes.truncate(MathConstants::BYTE_LEN - 2); // drop the trailing uint16
    let mut reader = Reader::new(&bytes);
    assert!(MathConstants::parse(&mut reader).is_err(), "a truncated record must not parse");
}

macro_rules! compare_value_records {
    ($ours:expr, $tt:expr; $($name:ident),* $(,)?) => {
        $(
            assert_eq!(
                $ours.$name,
                $tt.$name().value as f32,
                "MathConstants::{} differs from ttf-parser",
                stringify!($name)
            );
        )*
    };
}

fn compare_constants(ours: &MathConstants, tt: &ttf_parser::math::Constants<'_>) {
    assert_eq!(ours.script_percent_scale_down, tt.script_percent_scale_down() as f32);
    assert_eq!(ours.script_script_percent_scale_down, tt.script_script_percent_scale_down() as f32);
    assert_eq!(ours.delimited_sub_formula_min_height, tt.delimited_sub_formula_min_height() as f32);
    assert_eq!(ours.display_operator_min_height, tt.display_operator_min_height() as f32);
    compare_value_records!(
        ours, tt;
        math_leading,
        axis_height,
        accent_base_height,
        flattened_accent_base_height,
        subscript_shift_down,
        subscript_top_max,
        subscript_baseline_drop_min,
        superscript_shift_up,
        superscript_shift_up_cramped,
        superscript_bottom_min,
        superscript_baseline_drop_max,
        sub_superscript_gap_min,
        superscript_bottom_max_with_subscript,
        space_after_script,
        upper_limit_gap_min,
        upper_limit_baseline_rise_min,
        lower_limit_gap_min,
        lower_limit_baseline_drop_min,
        stack_top_shift_up,
        stack_top_display_style_shift_up,
        stack_bottom_shift_down,
        stack_bottom_display_style_shift_down,
        stack_gap_min,
        stack_display_style_gap_min,
        stretch_stack_top_shift_up,
        stretch_stack_bottom_shift_down,
        stretch_stack_gap_above_min,
        stretch_stack_gap_below_min,
        fraction_numerator_shift_up,
        fraction_numerator_display_style_shift_up,
        fraction_denominator_shift_down,
        fraction_denominator_display_style_shift_down,
        fraction_numerator_gap_min,
        fraction_num_display_style_gap_min,
        fraction_rule_thickness,
        fraction_denominator_gap_min,
        fraction_denom_display_style_gap_min,
        skewed_fraction_horizontal_gap,
        skewed_fraction_vertical_gap,
        overbar_vertical_gap,
        overbar_rule_thickness,
        overbar_extra_ascender,
        underbar_vertical_gap,
        underbar_rule_thickness,
        underbar_extra_descender,
        radical_vertical_gap,
        radical_display_style_vertical_gap,
        radical_rule_thickness,
        radical_extra_ascender,
        radical_kern_before_degree,
        radical_kern_after_degree,
    );
    assert_eq!(
        ours.radical_degree_bottom_raise_percent,
        tt.radical_degree_bottom_raise_percent() as f32
    );
}

#[test]
fn constants_match_ttf_parser_on_latin_modern_math() {
    let Some(data) = read_font(LMM_PATH)
    else {
        return;
    };
    let our_math = acorn_math(&data, 0);
    let theirs = tt_constants(&data, 0);
    compare_constants(&our_math.constants, &theirs);

    // Independent specification values for Latin Modern Math.
    let c = &our_math.constants;
    assert_eq!(c.script_percent_scale_down, 70.0);
    assert_eq!(c.script_script_percent_scale_down, 50.0);
    assert_eq!(c.axis_height, 250.0);
    assert!(c.axis_height > 0.0);
    assert_eq!(c.space_after_script, 56.0);
    assert_eq!(c.radical_degree_bottom_raise_percent, 60.0);
}

#[test]
fn constants_match_ttf_parser_on_cambria_math() {
    let Some(data) = read_font(CAMBRIA_TTC_PATH)
    else {
        return;
    };
    // Face 0 is Cambria, face 1 is Cambria Math (TrueType `glyf` outlines).
    let our_math = acorn_math(&data, 1);
    let theirs = tt_constants(&data, 1);
    compare_constants(&our_math.constants, &theirs);

    let c = &our_math.constants;
    assert_eq!(c.script_percent_scale_down, 73.0);
    assert_eq!(c.axis_height, 585.0);
    assert_eq!(c.space_after_script, 85.0);
    assert_eq!(c.radical_degree_bottom_raise_percent, 65.0);
}
