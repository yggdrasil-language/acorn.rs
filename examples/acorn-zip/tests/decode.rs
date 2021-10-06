#[path = "common/mod.rs"]
mod common;

use acorn_core::Budget;
use acorn_source::MemorySource;
use acorn_zip::{COMPRESSION_DEFLATE, decode_member_with_facts, index_zip_bytes, read_member_payload};

use acorn_codec::compression::deflate::encode_stored;

use common::minimal_single_file_zip;

fn deflate_zip(name: &[u8], payload: &[u8], compressed: &[u8]) -> Vec<u8> {
    let crc = crc32(payload);

    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&crc.to_le_bytes());
    archive.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive.extend_from_slice(name);
    archive.extend_from_slice(compressed);

    let cd_offset = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    // 42 bytes after signature (APPNOTE total fixed = 46).
    let mut cd_fixed = [0u8; 42];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[6..8].copy_from_slice(&[0x08, 0x00]);
    cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
    cd_fixed[16..20].copy_from_slice(&(compressed.len() as u32).to_le_bytes());
    cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
    cd_fixed[38..42].copy_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&cd_fixed);
    archive.extend_from_slice(name);

    let cd_size = archive.len() - cd_offset;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
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

fn minimal_deflated_zip() -> Vec<u8> {
    let payload = b"xy";
    deflate_zip(b"b", payload, &encode_stored(payload))
}

#[test]
fn decodes_deflated_member_payload() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    assert_eq!(member.compression_method, COMPRESSION_DEFLATE);
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let decoded = read_member_payload(source, member, &Budget::default()).expect("decode");
    assert_eq!(decoded, b"xy");
}

#[test]
fn stored_decode_matches_read_stored_payload() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("stored.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("stored.zip", bytes);
    let decoded = read_member_payload(source, member, &Budget::default()).expect("decode");
    assert_eq!(decoded, b"x");
}

#[test]
fn decode_facts_report_crc_and_full_consumption() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let (_decoded, facts) = decode_member_with_facts("m", source, member, &Budget::default()).expect("facts");
    assert!(facts.fully_consumed);
    assert_eq!(facts.crc32_expected, facts.crc32_actual);
    assert!(facts.checksum_ok());
    assert_eq!(facts.decoded_size, 2);
}
