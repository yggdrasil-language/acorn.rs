#[path = "common/mod.rs"]
mod common;

use acorn_source::MemorySource;
use acorn_zip::{index_zip_bytes, read_stored_payload, ZipViewError};

use common::minimal_single_file_zip;

#[test]
fn stored_member_payload_view_reads_bytes() {
    let bytes = minimal_single_file_zip();
    let index = index_zip_bytes("minimal.zip", bytes.clone()).expect("index");
    let member = &index.members[0];
    let source = MemorySource::from_bytes("minimal.zip", bytes);
    let payload = read_stored_payload(source, member).expect("payload");
    assert_eq!(payload, b"x");
}

#[test]
fn rejects_deflated_member_view_for_now() {
    let bytes = minimal_single_file_zip();
    let mut index = index_zip_bytes("minimal.zip", bytes.clone()).expect("index");
    index.members[0].compression_method = 8;
    let source = MemorySource::from_bytes("minimal.zip", bytes);
    let err = read_stored_payload(source, &index.members[0]).expect_err("deflate");
    assert!(matches!(err, ZipViewError::UnsupportedCompression(8)));
}
