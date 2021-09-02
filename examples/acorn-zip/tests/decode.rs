#[path = "common/mod.rs"]
mod common;

use acorn_core::ParseBudget;
use acorn_source::MemorySource;
use acorn_zip::{index_zip_bytes, read_member_payload, COMPRESSION_DEFLATE};

use common::minimal_single_file_zip;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Write;

fn raw_deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("compress");
    encoder.finish().expect("finish deflate")
}

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
    let mut cd_fixed = [0u8; 46];
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
    deflate_zip(b"b", payload, &raw_deflate(payload))
}

#[test]
fn decodes_deflated_member_payload() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    assert_eq!(member.compression_method, COMPRESSION_DEFLATE);
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let decoded = read_member_payload(source, member, &ParseBudget::default()).expect("decode");
    assert_eq!(decoded, b"xy");
}

#[test]
fn stored_decode_matches_read_stored_payload() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("stored.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("stored.zip", bytes);
    let decoded = read_member_payload(source, member, &ParseBudget::default()).expect("decode");
    assert_eq!(decoded, b"x");
}
