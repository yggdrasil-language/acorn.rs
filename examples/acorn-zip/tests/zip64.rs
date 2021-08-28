#[path = "common/mod.rs"]
mod common;

use acorn_zip::index_zip_bytes;

use common::minimal_single_file_zip;

#[test]
fn indexes_zip64_eocd_with_sentinel_fields() {
    let mut archive = minimal_single_file_zip();
    archive.truncate(archive.len() - 22);
    let local_size = 32u64;
    let cd_size = 51u64;
    let cd_offset = local_size;

    let zip64_eocd_offset = archive.len();
    archive.extend_from_slice(b"PK\x06\x06");
    archive.extend_from_slice(&56u64.to_le_bytes());
    archive.extend_from_slice(&[0x2d, 0x00, 0x2d, 0x00]);
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&1u64.to_le_bytes());
    archive.extend_from_slice(&1u64.to_le_bytes());
    archive.extend_from_slice(&cd_size.to_le_bytes());
    archive.extend_from_slice(&cd_offset.to_le_bytes());

    archive.extend_from_slice(b"PK\x06\x07");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
    archive.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    archive.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    archive.extend_from_slice(&[0x00, 0x00]);

    let index = index_zip_bytes("zip64.zip", archive).expect("index");
    assert!(index.eocd.zip64);
    assert_eq!(index.eocd.entry_count, 1);
    assert_eq!(index.eocd.central_directory_offset, cd_offset);
    assert_eq!(index.members.len(), 1);
}
