#[path = "common/mod.rs"]
mod common;

use acorn_core::{AddressSpaceId, NeedRange};
use acorn_source::{ByteSource, MemorySource, RangeMapSource, SourceIdentity};
use acorn_zip::{index_zip, index_zip_bytes};

use common::minimal_single_file_zip;

#[test]
fn indexes_minimal_zip_members() {
    let index = index_zip_bytes("minimal.zip", minimal_single_file_zip()).expect("index");
    assert_eq!(index.members.len(), 1);
    assert_eq!(index.members[0].path, "a");
    assert_eq!(index.members[0].compressed_size, 1);
    assert_eq!(index.members[0].crc32, 0x8cdc_1683);
    assert_eq!(index.members[0].version_needed, 20);
    assert_eq!(index.members[0].general_purpose_bit_flag, 0);
    assert!(index.layout.nodes().len() >= 2);
}

#[test]
fn rejects_non_zip_input() {
    let err = index_zip_bytes("not.zip", b"hello".to_vec()).expect_err("not zip");
    assert!(matches!(err, acorn_zip::ZipIndexError::NotZip));
}

#[test]
fn index_reuse_requires_the_same_content_digest() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("minimal.zip", bytes.clone()).expect("index");

    let same = MemorySource::from_bytes("minimal.zip", bytes.clone());
    assert!(index.reuses(&same.identity()));

    let mut changed = bytes;
    changed[4] = 0xFF;
    let different = MemorySource::from_bytes("minimal.zip", changed);
    assert!(!index.reuses(&different.identity()));
}

#[test]
fn index_reuse_rejects_a_renamed_source() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("minimal.zip", bytes.clone()).expect("index");
    let renamed = MemorySource::from_bytes("moved.zip", bytes);
    assert!(!index.reuses(&renamed.identity()));
}

#[test]
fn index_reuse_rejects_a_label_without_a_digest() {
    let index = index_zip_bytes("minimal.zip", minimal_single_file_zip()).expect("index");
    let labelled_only = SourceIdentity::new("minimal.zip");
    assert!(!index.reuses(&labelled_only));
}

#[test]
fn indexing_a_source_without_bytes_reports_the_missing_range() {
    let bytes = minimal_single_file_zip();
    let source = RangeMapSource::new("remote.zip", vec![]).with_total_length(bytes.len() as u64);
    let error = index_zip(&source).expect_err("the archive was not fetched");

    match error {
        acorn_zip::ZipIndexError::NeedRange { range } => {
            // Only the shortest tail window is asked for first, and it must end at EOF.
            assert_eq!(range.start + range.len, bytes.len() as u64);
            assert!(range.len >= 22, "the shortest window still holds a whole record");
        }
        other => panic!("expected a range need, got {other}"),
    }
}

#[test]
fn a_supplied_archive_indexes_as_usual() {
    let bytes = minimal_single_file_zip();
    let mut source = RangeMapSource::new("remote.zip", vec![]).with_total_length(bytes.len() as u64);
    source.provide_range(0, bytes.clone()).expect("supply the fetched archive");

    let index = index_zip(&source).expect("index");
    assert_eq!(index.members.len(), 1);
    assert_eq!(index.members[0].path, "a");
}

#[test]
fn indexing_reads_only_the_tail_directory_and_local_headers() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("minimal.zip", bytes.clone()).expect("index");
    let member = &index.members[0];

    // Everything except the stored payload byte: directory, tail record, and header.
    let payload = member.payload_range;
    let mut source = RangeMapSource::new("partial.zip", vec![]).with_total_length(bytes.len() as u64);
    source.provide_range(0, bytes[..payload.start as usize].to_vec()).expect("header region");
    source
        .provide_range(index.eocd.central_directory_offset, bytes[index.eocd.central_directory_offset as usize..].to_vec())
        .expect("directory and tail");

    let partial = index_zip(&source).expect("index without the payload");
    assert_eq!(partial.members.len(), 1);
    assert_eq!(partial.members[0].path, "a");
    assert_eq!(partial.members[0].payload_range, payload);
}

#[test]
fn indexing_reports_the_tail_range_when_it_is_missing() {
    let bytes = minimal_single_file_zip();
    // Only the payload byte is present, so the directory and the tail record are missing.
    let mut source = RangeMapSource::new("partial.zip", vec![]).with_total_length(bytes.len() as u64);
    source.provide_range(0, bytes[..1].to_vec()).expect("payload only");

    let error = index_zip(&source).expect_err("the tail record was not fetched");
    assert!(matches!(error, acorn_zip::ZipIndexError::NeedRange { .. }), "got {error}");
}

#[test]
fn a_missing_range_becomes_a_core_range_request() {
    let bytes = minimal_single_file_zip();
    let source = RangeMapSource::new("remote.zip", vec![]).with_total_length(bytes.len() as u64);
    let error = index_zip(&source).expect_err("the archive was not fetched");

    let request: NeedRange = error.need_range(AddressSpaceId(0), "fetch the archive tail").expect("range request");
    assert_eq!(request.space, AddressSpaceId(0));
    assert_eq!(request.range.start + request.range.len, bytes.len() as u64);
    assert_eq!(request.reason, "fetch the archive tail");

    assert!(acorn_zip::ZipIndexError::NotZip.need_range(AddressSpaceId(0), "unused").is_none());
}

#[test]
fn repeated_range_requests_converge_on_a_full_index() {
    let bytes = minimal_single_file_zip();
    let mut source = RangeMapSource::new("remote.zip", vec![]).with_total_length(bytes.len() as u64);
    let mut fetches = 0;

    loop {
        match index_zip(&source) {
            Ok(index) => {
                assert_eq!(index.members.len(), 1);
                assert_eq!(index.members[0].path, "a");
                assert!(fetches >= 2, "the tail, the directory, and a header are separate reads");
                break;
            }
            Err(error) => {
                let request = error.need_range(AddressSpaceId(0), "fetch the missing interval").expect("range request");
                fetches += 1;
                assert!(fetches <= 8, "the fetch loop must converge");
                let start = request.range.start as usize;
                let end = (request.range.start + request.range.len) as usize;
                source.provide_range(request.range.start, bytes[start..end].to_vec()).expect("supply the fetched interval");
            }
        }
    }
}

#[test]
fn indexes_an_archive_without_members() {
    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0u8; 18]);
    let index = index_zip_bytes("empty.zip", archive).expect("index");
    assert!(index.members.is_empty());
    assert_eq!(index.eocd.entry_count, 0);
    assert!(!index.layout.nodes().is_empty());
}

#[test]
fn rejects_a_member_payload_past_the_input() {
    // The header, directory, and tail record are intact, but the member declares two
    // payload bytes while the file ends after the first one.
    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&2u32.to_le_bytes());
    archive.extend_from_slice(&2u32.to_le_bytes());
    archive.extend_from_slice(&1u16.to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive.push(b'a');
    archive.push(b'x');

    let local_size = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    let mut cd_fixed = [0u8; 42];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[16..20].copy_from_slice(&2u32.to_le_bytes());
    cd_fixed[20..24].copy_from_slice(&2u32.to_le_bytes());
    cd_fixed[24..26].copy_from_slice(&1u16.to_le_bytes());
    cd_fixed[38..42].copy_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&cd_fixed);
    archive.push(b'a');

    let cd_size = archive.len() - local_size;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(local_size as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);

    let error = index_zip_bytes("truncated.zip", archive).expect_err("payload past the input");
    assert!(matches!(error, acorn_zip::ZipIndexError::MalformedEntry { .. }), "got {error}");
}
