//! `cmap` conformance and differential tests.
//!
//! Ground truth comes from two real fonts parsed by `ttf-parser`:
//! 1. Latin Modern Math (OTF/CFF, the font VuTeX ships).
//! 2. Cambria Math (TrueType `glyf`, face 1 of `cambria.ttc`).
//!
//! Glyph ids, horizontal advances and glyph boxes must agree glyph by glyph.
//! Cmap formats that neither font uses (0, 10, 13) and the precedence rules
//! are covered with synthetic payloads.

use acorn_ttf::{
    font::FontFace,
    sfnt::parse_font_validated,
    tables::{CmapSubtableData, CmapTable, cmap::ParseCmapError},
};
use std::collections::BTreeSet;

/// Latin Modern Math shipped with the VuTeX workspace.
const LMM_PATH: &str = r#"E:\dxo 深度学习\VuTeX\projects\crates\vutex-text\assets\LatinModernMath-Regular.otf"#;
/// TrueType-flavoured math font (face 1 of the Windows Cambria collection).
const CAMBRIA_TTC_PATH: &str = r"C:\Windows\Fonts\cambria.ttc";

/// Codepoints named by the VuTeX font migration report, plus a supplementary
/// plane pair that only a `format 12` subtable can answer.
const SAMPLE: &[char] = &['A', 'a', '0', '+', '=', '√', '∑', '∫', '∈', 'α', '∞', '→', '−', '\u{1D400}', '\u{1D7CA}'];

/// Samples every math font must map. `U+1D7CA` is Cambria-only, so it is only
/// compared and not required.
const REQUIRED: &[char] = &['A', 'a', '0', '+', '=', '√', '∑', '∫', '∈', 'α', '∞', '→', '−', '\u{1D400}'];

fn read_font(path: &str) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(data) => Some(data),
        Err(_) => {
            eprintln!("SKIP: font not available at {path}");
            None
        }
    }
}

fn acorn_face(data: &[u8], face_index: u32) -> FontFace {
    let container = parse_font_validated(data).expect("container");
    FontFace::from_face(container.face(face_index as usize).expect("face index").clone())
}

/// `ttf-parser` glyph id, with `.notdef` treated as "no mapping" so that both
/// parsers answer the same question.
fn ttf_gid(face: &ttf_parser::Face<'_>, cp: u32) -> Option<u16> {
    char::from_u32(cp).and_then(|ch| face.glyph_index(ch)).map(|gid| gid.0).filter(|gid| *gid != 0)
}

fn compare_font(path: &str, face_index: u32) {
    let Some(data) = read_font(path)
    else {
        return;
    };
    let mut font = acorn_face(&data, face_index);
    let tt = ttf_parser::Face::parse(&data, face_index).expect("ttf-parser face");
    let tt_glyphs = tt.number_of_glyphs();

    // 1. Every required sample must be mapped by both sides and agree.
    for ch in REQUIRED {
        let cp = *ch as u32;
        let ours = font.cmap().and_then(|cmap| cmap.lookup(cp));
        let theirs = ttf_gid(&tt, cp);
        assert!(theirs.is_some(), "{path}: ttf-parser does not map U+{cp:04X}");
        assert_eq!(ours, theirs, "{path}: U+{cp:04X} glyph id differs from ttf-parser");
    }

    // 2. Nothing we expose may be phantom, and every codepoint ttf-parser
    //    declares must be answered by us. This walks the whole repertoire of
    //    the font (BMP and supplementary planes) instead of a fixed range.
    let mut codepoints: BTreeSet<u32> = SAMPLE.iter().map(|ch| *ch as u32).collect();
    let ours_keys: Vec<u32> = font.cmap().map(|cmap| cmap.iter().map(|(cp, _)| cp).collect()).unwrap_or_default();
    codepoints.extend(ours_keys);
    if let Some(cmap) = tt.tables().cmap {
        for subtable in cmap.subtables {
            subtable.codepoints(|cp| {
                codepoints.insert(cp);
            });
        }
    }
    for cp in codepoints {
        let ours = font.cmap().and_then(|cmap| cmap.lookup(cp));
        assert_eq!(ours, ttf_gid(&tt, cp), "{path}: U+{cp:04X} glyph id differs from ttf-parser");
    }

    // 3. Same glyph id implies same advance and same box.
    let mut boxes = 0usize;
    for ch in SAMPLE {
        let cp = *ch as u32;
        let Some(gid) = font.glyph_index(*ch)
        else {
            continue;
        };
        assert_eq!(Some(gid), ttf_gid(&tt, cp), "{path}: U+{cp:04X} glyph id differs from ttf-parser");
        assert!(gid < tt_glyphs, "{path}: glyph {gid} is outside the font");

        let our_advance = font.glyph_hor_advance(gid);
        let their_advance = tt.glyph_hor_advance(ttf_parser::GlyphId(gid));
        assert_eq!(our_advance, their_advance, "{path}: glyph {gid} advance differs from ttf-parser");

        let our_box = font.glyph_bounding_box(gid);
        let their_box = tt.glyph_bounding_box(ttf_parser::GlyphId(gid));
        if let Some(bbox) = our_box {
            boxes += 1;
            let other = their_box.unwrap_or_else(|| panic!("{path}: ttf-parser has no box for glyph {gid}"));
            assert_eq!(
                (bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max),
                (other.x_min, other.y_min, other.x_max, other.y_max),
                "{path}: glyph {gid} box differs from ttf-parser"
            );
        }
    }
    if font.is_truetype_glyf() {
        // Composite and empty glyphs legitimately carry no box of their own,
        // but a TrueType font must still answer for most of the sample.
        assert!(boxes >= SAMPLE.len() / 2, "{path}: only {boxes} of {} sample glyphs had a box", SAMPLE.len());
    }
}

#[test]
fn cmap_matches_ttf_parser_on_latin_modern_math() {
    compare_font(LMM_PATH, 0);
}

#[test]
fn cmap_matches_ttf_parser_on_cambria_math() {
    compare_font(CAMBRIA_TTC_PATH, 1);
}

#[test]
fn macintosh_subtable_never_overrides_unicode() {
    let Some(data) = read_font(LMM_PATH)
    else {
        return;
    };
    let mut font = acorn_face(&data, 0);
    let cmap = font.cmap().expect("cmap").clone();

    let mac = cmap.subtables.iter().find(|sub| sub.platform == 1).expect("LMM carries a Macintosh subtable");
    assert_eq!(mac.format, 6, "LMM Macintosh record is a trimmed table mapping");
    let mac_map = mac.map().expect("format 6 parses");

    let unicode_maps: Vec<_> = cmap.subtables.iter().filter(|sub| sub.platform != 1).filter_map(|sub| sub.map()).collect();
    let mut overlapping = 0usize;
    let mut mac_only = Vec::new();
    for (&cp, _) in mac_map {
        match unicode_maps.iter().find_map(|map| map.get(&cp)) {
            Some(&expected) => {
                overlapping += 1;
                assert_eq!(cmap.unified.get(&cp), Some(&expected), "Macintosh subtable overrode U+{cp:04X}");
            }
            None => mac_only.push(cp),
        }
    }
    assert!(overlapping > 0, "no codepoint overlap to check precedence on");
    assert!(!mac_only.is_empty(), "LMM Macintosh subtable has no Mac-only byte code");
    for cp in mac_only {
        assert!(!cmap.unified.contains_key(&cp), "Mac-only byte code U+{cp:04X} leaked into the unified map");
    }
}

#[test]
fn unsupported_cmap_format_is_reported_not_empty() {
    let Some(data) = read_font(CAMBRIA_TTC_PATH)
    else {
        return;
    };
    let mut font = acorn_face(&data, 1);
    let cmap = font.cmap().expect("cmap");
    let unsupported: Vec<u16> =
        cmap.subtables.iter().filter(|sub| matches!(sub.data, CmapSubtableData::Unsupported)).map(|sub| sub.format).collect();
    // Cambria Math carries a format 14 Unicode variation sequence subtable.
    assert_eq!(unsupported, vec![14], "unsupported subtable formats");
    assert!(cmap.records.iter().any(|rec| rec.platform == 0 && rec.encoding == 5), "format 14 record stays visible");
}

// ---- synthetic cmap payloads ----

/// Assemble a `cmap` table whose records point at the given subtables.
fn cmap_with(records: &[(u16, u16)], subtables: &[Vec<u8>]) -> Vec<u8> {
    assert_eq!(records.len(), subtables.len());
    let mut header = Vec::new();
    header.extend_from_slice(&0u16.to_be_bytes());
    header.extend_from_slice(&(records.len() as u16).to_be_bytes());
    let mut offset = 4 + records.len() * 8;
    for ((platform, encoding), subtable) in records.iter().zip(subtables) {
        header.extend_from_slice(&platform.to_be_bytes());
        header.extend_from_slice(&encoding.to_be_bytes());
        header.extend_from_slice(&(offset as u32).to_be_bytes());
        offset += subtable.len();
    }
    let mut out = header;
    for subtable in subtables {
        out.extend_from_slice(subtable);
    }
    out
}

fn u16s(values: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes
}

/// A single 0x41..=0x43 segment that reads `glyphIdArray` through
/// `idRangeOffset`, so the self-relative addressing is exercised.
fn format4_subtable() -> Vec<u8> {
    let mut subtable = Vec::new();
    subtable.extend(u16s(&[4, 0, 0, 4, 0, 0, 0])); // format, length, language, segCountX2, search/entry/range
    subtable.extend(u16s(&[0x43, 0xFFFF])); // endCode
    subtable.extend(u16s(&[0])); // reservedPad
    subtable.extend(u16s(&[0x41, 0xFFFF])); // startCode
    subtable.extend(u16s(&[0, 1])); // idDelta (0 for the real segment, 1 for the sentinel)
    // idRangeOffset starts at byte 28 and glyphIdArray at byte 32, so entry 0
    // is four bytes ahead of its own address.
    subtable.extend(u16s(&[4, 0]));
    subtable.extend(u16s(&[10, 0, 12])); // glyphIdArray: 0x41 -> 10, 0x42 -> missing, 0x43 -> 12
    subtable
}

fn format0_subtable() -> Vec<u8> {
    let mut subtable = u16s(&[0, 262, 0]);
    let mut glyphs = [0u8; 256];
    glyphs[0x41] = 7;
    subtable.extend_from_slice(&glyphs);
    subtable
}

fn format6_subtable() -> Vec<u8> {
    let mut subtable = u16s(&[6, 12, 0, 0x41, 3]);
    subtable.extend(u16s(&[5, 0, 7]));
    subtable
}

fn format10_subtable() -> Vec<u8> {
    let mut subtable = u16s(&[10, 0]);
    subtable.extend_from_slice(&32u32.to_be_bytes()); // length
    subtable.extend_from_slice(&0u32.to_be_bytes()); // language
    subtable.extend_from_slice(&0x1F600u32.to_be_bytes()); // firstCode
    subtable.extend_from_slice(&2u32.to_be_bytes()); // entryCount
    subtable.extend(u16s(&[9, 10]));
    subtable
}

fn format12_subtable() -> Vec<u8> {
    let mut subtable = u16s(&[12, 0]);
    subtable.extend_from_slice(&28u32.to_be_bytes()); // length
    subtable.extend_from_slice(&0u32.to_be_bytes()); // language
    subtable.extend_from_slice(&1u32.to_be_bytes()); // numGroups
    subtable.extend_from_slice(&0x1D400u32.to_be_bytes());
    subtable.extend_from_slice(&0x1D402u32.to_be_bytes());
    subtable.extend_from_slice(&0x100u32.to_be_bytes());
    subtable
}

fn format13_subtable() -> Vec<u8> {
    let mut subtable = u16s(&[13, 0]);
    subtable.extend_from_slice(&28u32.to_be_bytes()); // length
    subtable.extend_from_slice(&0u32.to_be_bytes()); // language
    subtable.extend_from_slice(&1u32.to_be_bytes()); // numGroups
    subtable.extend_from_slice(&0x41u32.to_be_bytes());
    subtable.extend_from_slice(&0x43u32.to_be_bytes());
    subtable.extend_from_slice(&77u32.to_be_bytes()); // one glyph for the whole range
    subtable
}

fn parse_single(format: u16, subtable: Vec<u8>) -> CmapTable {
    let data = cmap_with(&[(3, 1)], &[subtable]);
    let cmap = CmapTable::parse(&data).expect("cmap parses");
    assert_eq!(cmap.subtables.len(), 1);
    assert_eq!(cmap.subtables[0].format, format);
    cmap
}

#[test]
fn format_4_reads_glyph_id_array_relative_to_its_own_entry() {
    let cmap = parse_single(4, format4_subtable());
    assert_eq!(cmap.lookup(0x41), Some(10));
    assert_eq!(cmap.lookup(0x42), None, "glyphIdArray zero means no mapping");
    assert_eq!(cmap.lookup(0x43), Some(12));
}

#[test]
fn format_0_and_format_6_are_dense_byte_tables() {
    let zero = parse_single(0, format0_subtable());
    assert_eq!(zero.lookup(0x41), Some(7));
    assert_eq!(zero.lookup(0x42), None);
    assert_eq!(zero.lookup(0x100), None, "format 0 only covers the first 256 codes");

    let six = parse_single(6, format6_subtable());
    assert_eq!(six.lookup(0x41), Some(5));
    assert_eq!(six.lookup(0x42), None, "zero glyph id in a trimmed table means no mapping");
    assert_eq!(six.lookup(0x43), Some(7));
    assert_eq!(six.lookup(0x44), None, "trimmed table stops after entryCount");
}

#[test]
fn format_10_is_a_trimmed_32_bit_array() {
    let cmap = parse_single(10, format10_subtable());
    assert_eq!(cmap.lookup(0x1F600), Some(9));
    assert_eq!(cmap.lookup(0x1F601), Some(10));
    assert_eq!(cmap.lookup(0x1F602), None);
    assert_eq!(cmap.lookup(0x41), None);
}

#[test]
fn format_12_skips_the_reserved_field_and_keeps_wide_glyph_ids() {
    let cmap = parse_single(12, format12_subtable());
    assert_eq!(cmap.lookup(0x1D400), Some(256));
    assert_eq!(cmap.lookup(0x1D401), Some(257));
    assert_eq!(cmap.lookup(0x1D402), Some(258));
    assert_eq!(cmap.lookup(0x1D403), None);
}

#[test]
fn format_13_repeats_one_glyph_across_a_range() {
    let cmap = parse_single(13, format13_subtable());
    assert_eq!(cmap.lookup(0x41), Some(77));
    assert_eq!(cmap.lookup(0x42), Some(77));
    assert_eq!(cmap.lookup(0x43), Some(77));
    assert_eq!(cmap.lookup(0x44), None);
}

#[test]
fn format_12_rejects_glyph_ids_above_u16() {
    let mut subtable = u16s(&[12, 0]);
    subtable.extend_from_slice(&28u32.to_be_bytes());
    subtable.extend_from_slice(&0u32.to_be_bytes());
    subtable.extend_from_slice(&1u32.to_be_bytes());
    subtable.extend_from_slice(&0x41u32.to_be_bytes());
    subtable.extend_from_slice(&0x41u32.to_be_bytes());
    subtable.extend_from_slice(&0x1_0000u32.to_be_bytes());
    let data = cmap_with(&[(3, 10)], &[subtable]);
    match CmapTable::parse(&data) {
        Err(ParseCmapError::Malformed(message)) => assert!(message.contains("exceeds u16"), "message={message}"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn unknown_format_stays_visible_as_unsupported() {
    let data = cmap_with(&[(0, 5)], &[u16s(&[14])]);
    let cmap = CmapTable::parse(&data).expect("cmap parses");
    assert_eq!(cmap.subtables.len(), 1);
    assert_eq!(cmap.subtables[0].format, 14);
    assert!(matches!(cmap.subtables[0].data, CmapSubtableData::Unsupported));
    assert!(cmap.subtables[0].map().is_none());
    assert!(cmap.unified.is_empty());
}

#[test]
fn windows_unicode_subtable_wins_over_macintosh() {
    // Both subtables claim `A`. The Macintosh one must not answer for it.
    let mut mac = u16s(&[6, 12, 0, 0x41, 1]);
    mac.extend(u16s(&[99]));
    let data = cmap_with(&[(1, 0), (3, 1)], &[mac.clone(), format4_subtable()]);
    let cmap = CmapTable::parse(&data).expect("cmap parses");
    assert_eq!(cmap.lookup(0x41), Some(10));
    assert_eq!(cmap.lookup(0x43), Some(12));

    // Swapping the record order must not change the winner either.
    let data = cmap_with(&[(3, 1), (1, 0)], &[format4_subtable(), mac]);
    let cmap = CmapTable::parse(&data).expect("cmap parses");
    assert_eq!(cmap.lookup(0x41), Some(10));
}
