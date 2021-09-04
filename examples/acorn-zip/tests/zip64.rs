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

#[test]
fn indexes_zip64_member_extra_uncompressed_size() {
    let payload = vec![b'x'; 512];
    let name = b"a";
    let crc = crc32(&payload);
    let uncompressed = payload.len() as u64;
    let zip64_extra = zip64_extra_uncompressed(uncompressed);

    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&crc.to_le_bytes());
    archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive.extend_from_slice(name);
    archive.extend_from_slice(&payload);

    let cd_offset = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    let mut cd_fixed = [0u8; 46];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
    cd_fixed[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    cd_fixed[20..24].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
    cd_fixed[26..28].copy_from_slice(&(zip64_extra.len() as u16).to_le_bytes());
    cd_fixed[38..42].copy_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&cd_fixed);
    archive.extend_from_slice(name);
    archive.extend_from_slice(&zip64_extra);

    let cd_size = archive.len() - cd_offset;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);

    let index = index_zip_bytes("zip64-member.zip", archive).expect("index");
    assert_eq!(index.members.len(), 1);
    assert_eq!(index.members[0].uncompressed_size, uncompressed);
    assert_eq!(index.members[0].compressed_size, uncompressed);
}

fn zip64_extra_uncompressed(uncompressed: u64) -> Vec<u8> {
    let mut extra = Vec::new();
    extra.extend_from_slice(&0x0001u16.to_le_bytes());
    extra.extend_from_slice(&8u16.to_le_bytes());
    extra.extend_from_slice(&uncompressed.to_le_bytes());
    extra
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
