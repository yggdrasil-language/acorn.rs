#[path = "common/mod.rs"]
mod common;

use acorn_zip::index_zip_bytes;

use common::minimal_single_file_zip;

#[test]
fn indexes_minimal_zip_members() {
    let index = index_zip_bytes("minimal.zip", minimal_single_file_zip()).expect("index");
    assert_eq!(index.members.len(), 1);
    assert_eq!(index.members[0].path, "a");
    assert_eq!(index.members[0].compressed_size, 1);
    assert!(index.layout.nodes().len() >= 2);
}

#[test]
fn rejects_non_zip_input() {
    let err = index_zip_bytes("not.zip", b"hello".to_vec()).expect_err("not zip");
    assert!(matches!(err, acorn_zip::ZipIndexError::NotZip));
}
