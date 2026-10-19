#[path = "common/mod.rs"]
mod common;

use acorn_codec::CodecError;
use acorn_core::{AddressSpaceId, Budget, BudgetTracker, ByteRange};
use acorn_source::{ByteSource, MemorySource, RangeMapSource};
use acorn_zip::{COMPRESSION_DEFLATE, ZipViewError, decode_member_with_facts, index_zip_bytes, read_member_payload};

use acorn_codec::compression::deflate::{Deflate, DeflateLevel};

use common::minimal_single_file_zip;

fn deflate_zip(name: &[u8], payload: &[u8], compressed: &[u8]) -> Vec<u8> {
    deflate_zip_declaring(name, compressed, payload.len() as u32, crc32(payload))
}

fn deflate_zip_declaring(name: &[u8], compressed: &[u8], declared_uncompressed: u32, declared_crc: u32) -> Vec<u8> {
    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&declared_crc.to_le_bytes());
    archive.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    archive.extend_from_slice(&declared_uncompressed.to_le_bytes());
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
    cd_fixed[12..16].copy_from_slice(&declared_crc.to_le_bytes());
    cd_fixed[16..20].copy_from_slice(&(compressed.len() as u32).to_le_bytes());
    cd_fixed[20..24].copy_from_slice(&declared_uncompressed.to_le_bytes());
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

fn empty_stored_zip() -> Vec<u8> {
    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    archive.push(b'e');

    let local_size = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    let mut cd_fixed = [0u8; 42];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[24..26].copy_from_slice(&[0x01, 0x00]);
    cd_fixed[38..42].copy_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&cd_fixed);
    archive.push(b'e');

    let cd_size = archive.len() - local_size;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(local_size as u32).to_le_bytes());
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
    deflate_zip(b"b", payload, &Deflate::encode(payload, DeflateLevel::Default))
}

#[test]
fn decodes_deflated_member_payload() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    assert_eq!(member.compression_method, COMPRESSION_DEFLATE);
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let decoded = read_member_payload(source, member, &mut BudgetTracker::new(Budget::default())).expect("decode");
    assert_eq!(decoded, b"xy");
}

#[test]
fn stored_decode_matches_read_stored_payload() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("stored.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("stored.zip", bytes);
    let decoded = read_member_payload(source, member, &mut BudgetTracker::new(Budget::default())).expect("decode");
    assert_eq!(decoded, b"x");
}

#[test]
fn decode_facts_report_crc_and_full_consumption() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let (_decoded, facts) =
        decode_member_with_facts("m", source, member, &mut BudgetTracker::new(Budget::default())).expect("facts");
    assert!(facts.fully_consumed);
    assert_eq!(facts.crc32_expected, facts.crc32_actual);
    assert!(facts.checksum_ok());
    assert_eq!(facts.decoded_size, 2);
}

#[test]
fn empty_member_decodes_to_empty_payload() {
    let bytes = empty_stored_zip();
    let index = index_zip_bytes("empty.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    assert_eq!(member.compressed_size, 0);
    assert_eq!(member.uncompressed_size, 0);
    let source = MemorySource::from_bytes("empty.zip", bytes);
    let (_decoded, facts) =
        decode_member_with_facts("e", source, member, &mut BudgetTracker::new(Budget::default())).expect("facts");
    assert_eq!(facts.decoded_size, 0);
    assert_eq!(facts.crc32_actual, 0);
    assert!(facts.checksum_ok());
}

#[test]
fn oversized_declared_compressed_size_is_rejected_before_reading() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: member.compressed_size - 1, ..Budget::default() });
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error = decode_member_with_facts("m", source, member, &mut ledger).expect_err("must reject");
    assert_eq!(error, ZipViewError::BudgetExceeded);
}

#[test]
fn oversized_declared_uncompressed_size_is_rejected_before_reading() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let mut ledger = BudgetTracker::new(Budget { max_decoded_bytes: 1, ..Budget::default() });
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error = decode_member_with_facts("m", source, member, &mut ledger).expect_err("must reject");
    assert_eq!(error, ZipViewError::BudgetExceeded);
}

#[test]
fn declared_uncompressed_size_mismatch_is_rejected() {
    let payload = b"xy";
    let bytes =
        deflate_zip_declaring(b"b", &Deflate::encode(payload, DeflateLevel::Default), payload.len() as u32 + 1, crc32(payload));
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error =
        decode_member_with_facts("m", source, member, &mut BudgetTracker::new(Budget::default())).expect_err("must reject");
    assert_eq!(error, ZipViewError::SizeMismatch);
}

#[test]
fn zero_declared_size_rejects_non_empty_payload() {
    let payload = b"xy";
    let bytes = deflate_zip_declaring(b"b", &Deflate::encode(payload, DeflateLevel::Default), 0, crc32(payload));
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error =
        decode_member_with_facts("m", source, member, &mut BudgetTracker::new(Budget::default())).expect_err("must reject");
    assert_eq!(error, ZipViewError::SizeMismatch);
}

#[test]
fn truncated_deflate_payload_preserves_codec_category() {
    let payload = b"xy";
    let encoded = Deflate::encode(payload, DeflateLevel::Default);
    let truncated = &encoded[..encoded.len() - 1];
    let bytes = deflate_zip_declaring(b"b", truncated, payload.len() as u32, crc32(payload));
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error =
        decode_member_with_facts("m", source, member, &mut BudgetTracker::new(Budget::default())).expect_err("must reject");
    match error {
        ZipViewError::Codec(codec_error) => {
            assert!(
                matches!(codec_error, CodecError::InputExhausted { .. } | CodecError::InvalidBlockHeader { .. }),
                "unexpected codec category: {codec_error}"
            );
        }
        other => panic!("codec category was folded away: {other}"),
    }
}

#[test]
fn member_decode_reports_a_range_that_can_be_supplied() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let payload = member.payload_range;

    let partial = RangeMapSource::new("deflate.zip", vec![]).with_total_length(bytes.len() as u64);
    let error = decode_member_with_facts("m", partial, member, &mut BudgetTracker::new(Budget::default()))
        .expect_err("payload was not fetched");
    match error {
        ZipViewError::NeedRange { range } => {
            assert_eq!(range, ByteRange::new_allowing_empty(payload.start, payload.len).expect("range"));
        }
        other => panic!("expected a range need, got {other}"),
    }

    let fetched = bytes[payload.start as usize..(payload.start + payload.len) as usize].to_vec();
    let mut partial = RangeMapSource::new("deflate.zip", vec![]).with_total_length(bytes.len() as u64);
    partial.provide_range(payload.start, fetched).expect("supply the fetched payload");

    let (decoded, facts) =
        decode_member_with_facts("m", partial, member, &mut BudgetTracker::new(Budget::default())).expect("decode");
    assert!(facts.checksum_ok());
    assert_eq!(decoded.length(), Some(2));
}

#[test]
fn one_ledger_covers_every_member_read() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = index.members[0].clone();

    let limits = Budget { max_read_bytes: member.compressed_size + member.compressed_size / 2, ..Budget::default() };
    let mut ledger = BudgetTracker::new(limits);
    let source = || MemorySource::from_bytes("deflate.zip", bytes.clone());

    decode_member_with_facts("m", source(), &member, &mut ledger).expect("first member");
    assert_eq!(ledger.usage().read_bytes, member.compressed_size);
    assert_eq!(ledger.usage().decoded_bytes, member.uncompressed_size);
    assert_eq!(ledger.reserved_read_bytes(), 0);
    assert_eq!(ledger.reserved_decoded_bytes(), 0);

    let error = decode_member_with_facts("m", source(), &member, &mut ledger).expect_err("the allowance is shared");
    assert_eq!(error, ZipViewError::BudgetExceeded);
}

#[test]
fn a_failed_member_settles_what_it_read_and_releases_the_rest() {
    let payload = b"xy";
    let encoded = Deflate::encode(payload, DeflateLevel::Default);
    let truncated = &encoded[..encoded.len() - 1];
    let bytes = deflate_zip_declaring(b"b", truncated, payload.len() as u32, crc32(payload));
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];

    let mut ledger = BudgetTracker::new(Budget::default());
    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error = decode_member_with_facts("m", source, member, &mut ledger).expect_err("codec failure");
    assert!(matches!(error, ZipViewError::Codec(_)));

    assert_eq!(ledger.usage().read_bytes, member.compressed_size, "a failed read still settles what it read");
    assert_eq!(ledger.reserved_read_bytes(), 0);
    assert_eq!(ledger.reserved_decoded_bytes(), 0);
}

#[test]
fn a_missing_member_range_becomes_a_core_range_request() {
    let bytes = minimal_deflated_zip();
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];

    let partial = RangeMapSource::new("deflate.zip", vec![]).with_total_length(bytes.len() as u64);
    let error = decode_member_with_facts("m", partial, member, &mut BudgetTracker::new(Budget::default()))
        .expect_err("payload was not fetched");

    let request = error.need_range(AddressSpaceId(0), "fetch the member payload").expect("range request");
    assert_eq!(request.space, AddressSpaceId(0));
    assert_eq!(
        request.range,
        ByteRange::new_allowing_empty(member.payload_range.start, member.payload_range.len).expect("range")
    );
    assert_eq!(request.reason, "fetch the member payload");
}

#[test]
fn reports_a_member_crc_mismatch() {
    let payload = b"xy";
    let bytes =
        deflate_zip_declaring(b"b", &Deflate::encode(payload, DeflateLevel::Default), payload.len() as u32, crc32(b"other"));
    let index = index_zip_bytes("deflate.zip", bytes.clone()).expect("index");
    let member = &index.members[0];

    let source = MemorySource::from_bytes("deflate.zip", bytes);
    let error =
        decode_member_with_facts("m", source, member, &mut BudgetTracker::new(Budget::default())).expect_err("crc mismatch");
    match error {
        ZipViewError::CrcMismatch { expected, actual } => {
            assert_eq!(expected, crc32(b"other"));
            assert_eq!(actual, crc32(payload));
        }
        other => panic!("expected a crc mismatch, got {other}"),
    }
}
