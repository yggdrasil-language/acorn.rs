use acorn_core::ParseBudget;
use acorn_docx::{parse_relationship_targets, OpcPackage};

fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut locals = Vec::new();

    for (path, payload) in entries {
        let name = path.as_bytes();
        let crc = crc32(payload);
        let local_offset = archive.len() as u32;

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&[0x00, 0x00]);
        archive.extend_from_slice(name);
        archive.extend_from_slice(payload);

        locals.push((name, crc, payload.len() as u32, local_offset));
    }

    let cd_offset = archive.len() as u32;
    for (name, crc, size, local_offset) in locals {
        archive.extend_from_slice(b"PK\x01\x02");
        let mut cd_fixed = [0u8; 46];
        cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
        cd_fixed[16..20].copy_from_slice(&size.to_le_bytes());
        cd_fixed[20..24].copy_from_slice(&size.to_le_bytes());
        cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
        cd_fixed[38..42].copy_from_slice(&local_offset.to_le_bytes());
        archive.extend_from_slice(&cd_fixed);
        archive.extend_from_slice(name);
    }

    let cd_size = archive.len() - cd_offset as usize;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&cd_offset.to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let bit = crc & 1;
            crc >>= 1;
            if bit != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

const DOCUMENT_RELS: &[u8] = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#;

#[test]
fn parses_relationship_targets_from_xml() {
    let targets = parse_relationship_targets(DOCUMENT_RELS).expect("parse");
    assert_eq!(
        targets.get("rId1").map(String::as_str),
        Some("https://example.com")
    );
    assert_eq!(
        targets.get("rId2").map(String::as_str),
        Some("media/image1.png")
    );
}

#[test]
fn reads_document_relationships_from_package() {
    let zip = stored_zip(&[
        ("word/document.xml", b"<w:document/>"),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS),
    ]);
    let pkg = OpcPackage::open("docx.zip", zip).expect("open");
    let budget = ParseBudget::default();
    let targets = pkg
        .read_relationship_targets("word/_rels/document.xml.rels", &budget)
        .expect("rels");
    assert_eq!(targets.len(), 2);
    assert_eq!(
        targets.get("rId2").map(String::as_str),
        Some("media/image1.png")
    );
}

#[test]
fn missing_relationships_part_returns_empty_map() {
    let zip = stored_zip(&[("word/document.xml", b"<w:document/>")]);
    let pkg = OpcPackage::open("docx.zip", zip).expect("open");
    let targets = pkg
        .read_relationship_targets("word/_rels/document.xml.rels", &ParseBudget::default())
        .expect("rels");
    assert!(targets.is_empty());
}
