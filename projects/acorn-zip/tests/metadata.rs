use acorn_zip::{
    FLAG_ENCRYPTED, FLAG_STRONG_ENCRYPTION, FLAG_UTF8_NAME, ZipDateTime, ZipEncryption, ZipNameEncoding, index_zip_bytes,
    parse_extended_timestamp,
};

fn ieee_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            }
            else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn put16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend(value.to_le_bytes());
}

fn put32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend(value.to_le_bytes());
}

/// Builds a stored single-member archive with the supplied header facts.
#[allow(clippy::too_many_arguments)]
fn stored_zip(
    name: &[u8],
    flags: u16,
    version_made_by: u16,
    modified_time: u16,
    modified_date: u16,
    external_attributes: u32,
    extra: &[u8],
    payload: &[u8],
) -> Vec<u8> {
    let crc = ieee_crc32(payload);
    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    put16(&mut archive, 20);
    put16(&mut archive, flags);
    put16(&mut archive, 0);
    put16(&mut archive, modified_time);
    put16(&mut archive, modified_date);
    put32(&mut archive, crc);
    put32(&mut archive, payload.len() as u32);
    put32(&mut archive, payload.len() as u32);
    put16(&mut archive, name.len() as u16);
    put16(&mut archive, extra.len() as u16);
    archive.extend_from_slice(name);
    archive.extend_from_slice(extra);
    archive.extend_from_slice(payload);

    let local_size = archive.len() as u32;
    archive.extend_from_slice(b"PK\x01\x02");
    put16(&mut archive, version_made_by);
    put16(&mut archive, 20);
    put16(&mut archive, flags);
    put16(&mut archive, 0);
    put16(&mut archive, modified_time);
    put16(&mut archive, modified_date);
    put32(&mut archive, crc);
    put32(&mut archive, payload.len() as u32);
    put32(&mut archive, payload.len() as u32);
    put16(&mut archive, name.len() as u16);
    put16(&mut archive, extra.len() as u16);
    put16(&mut archive, 0);
    put16(&mut archive, 0);
    put16(&mut archive, 0);
    put32(&mut archive, external_attributes);
    put32(&mut archive, 0);
    archive.extend_from_slice(name);
    archive.extend_from_slice(extra);

    let cd_offset = local_size;
    let cd_size = archive.len() as u32 - local_size;
    archive.extend_from_slice(b"PK\x05\x06");
    put16(&mut archive, 0);
    put16(&mut archive, 0);
    put16(&mut archive, 1);
    put16(&mut archive, 1);
    put32(&mut archive, cd_size);
    put32(&mut archive, cd_offset);
    put16(&mut archive, 0);
    archive
}

fn dos_words(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> (u16, u16) {
    let date = ((year - 1980) << 9) | ((month as u16) << 5) | day as u16;
    let time = ((hour as u16) << 11) | ((minute as u16) << 5) | (second as u16 >> 1);
    (time, date)
}

#[test]
fn reads_dos_time_and_directory_markers() {
    let (time, date) = dos_words(2026, 11, 7, 12, 34, 56);
    let bytes = stored_zip(b"folder/", 0, 20, time, date, 0x10, &[], b"");
    let index = index_zip_bytes("time.zip", bytes).expect("index");
    let member = &index.members[0];

    assert_eq!(member.modified.time, time);
    assert_eq!(member.modified.date, date);
    assert_eq!(member.modified.parts(), Some(ZipDateTime::new(2026, 11, 7, 12, 34, 56).unwrap()));
    assert_eq!(member.dos_attributes(), 0x10);
    assert!(member.name_is_directory());
    assert!(member.attributes_are_directory());
    assert!(member.is_directory());
}

#[test]
fn keeps_raw_name_bytes_and_reports_the_name_encoding() {
    let legacy_name = [0x82, b'.', b't', b'x', b't'];
    let legacy = index_zip_bytes("legacy.zip", stored_zip(&legacy_name, 0, 20, 0, 0, 0, &[], b"x")).expect("index");
    assert_eq!(legacy.members[0].raw_name, legacy_name);
    assert_eq!(legacy.members[0].name_encoding(), ZipNameEncoding::Unspecified);

    let utf8 =
        index_zip_bytes("utf8.zip", stored_zip("中文.txt".as_bytes(), FLAG_UTF8_NAME, 20, 0, 0, 0, &[], b"x")).expect("index");
    assert_eq!(utf8.members[0].name_encoding(), ZipNameEncoding::Utf8);
    assert_eq!(std::str::from_utf8(&utf8.members[0].raw_name), Ok("中文.txt"));
}

#[test]
fn classifies_encryption_from_general_purpose_flags() {
    let plain = index_zip_bytes("plain.zip", stored_zip(b"a", 0, 20, 0, 0, 0, &[], b"x")).expect("index");
    assert_eq!(plain.members[0].encryption(), ZipEncryption::None);
    assert!(!plain.members[0].is_encrypted());

    let traditional =
        index_zip_bytes("traditional.zip", stored_zip(b"a", FLAG_ENCRYPTED, 20, 0, 0, 0, &[], b"x")).expect("index");
    assert_eq!(traditional.members[0].encryption(), ZipEncryption::Traditional);

    let strong =
        index_zip_bytes("strong.zip", stored_zip(b"a", FLAG_ENCRYPTED | FLAG_STRONG_ENCRYPTION, 20, 0, 0, 0, &[], b"x"))
            .expect("index");
    assert_eq!(strong.members[0].encryption(), ZipEncryption::Strong);
}

#[test]
fn reads_unix_mode_from_external_attributes() {
    let version_made_by = (3u16 << 8) | 20;
    let mode = 0o040755u32;
    let bytes = stored_zip(b"dir/", 0, version_made_by, 0, 0, mode << 16, &[], b"");
    let index = index_zip_bytes("unix.zip", bytes).expect("index");
    let member = &index.members[0];

    assert_eq!(member.unix_mode(), Some(mode));
    assert!(member.attributes_are_directory());
    assert!(member.is_directory());
}

#[test]
fn reads_extended_timestamp_from_the_extra_field() {
    let modified: i32 = 1_791_398_400;
    let extra = [0x55, 0x54, 5, 0, 0x01, modified as u8, (modified >> 8) as u8, (modified >> 16) as u8, (modified >> 24) as u8];
    let bytes = stored_zip(b"a", 0, 20, 0, 0, 0, &extra, b"x");
    let index = index_zip_bytes("timestamp.zip", bytes).expect("index");

    assert_eq!(index.members[0].modified_unix, Some(i64::from(modified)));
    let parsed = parse_extended_timestamp(&extra, 0, extra.len()).expect("timestamp extra");
    assert_eq!(parsed.modified_unix, Some(i64::from(modified)));
    assert_eq!(parsed.accessed_unix, None);
    assert_eq!(parsed.created_unix, None);
}
