use acorn_core::{Budget, BudgetTracker};
use acorn_source::MemorySource;
use acorn_zip::{
    Zip64Mode, ZipCompression, ZipDateTime, ZipDosDateTime, ZipEntry, ZipNameEncoding, ZipWriteError, ZipWriter,
    index_zip_bytes, read_member_payload,
};

#[test]
fn writes_stored_and_deflated_members() {
    let stored_payload = b"alpha".to_vec();
    let deflated_payload = "compress me ".repeat(120).into_bytes();
    let mut writer = ZipWriter::new();
    writer.push(ZipEntry::stored("stored.txt", stored_payload.clone()));
    writer.push(ZipEntry::deflated("deflated.txt", deflated_payload.clone()));
    let bytes = writer.into_bytes().expect("write");

    let index = index_zip_bytes("writer.zip", bytes.clone()).expect("index");
    assert_eq!(index.members.len(), 2);
    assert_eq!(index.members[0].raw_name, b"stored.txt".to_vec());
    assert_eq!(index.members[0].compression_method, 0);
    assert_eq!(index.members[1].raw_name, b"deflated.txt".to_vec());
    assert_eq!(index.members[1].compression_method, 8);
    assert!(index.members[1].compressed_size < deflated_payload.len() as u64);
    assert_eq!(index.members[1].name_encoding(), ZipNameEncoding::Utf8);

    for (member, expected) in index.members.iter().zip([stored_payload, deflated_payload]) {
        let source = MemorySource::from_bytes("writer.zip", bytes.clone());
        let decoded = read_member_payload(source, member, &mut BudgetTracker::new(Budget::default())).expect("decode");
        assert_eq!(decoded, expected);
    }
}

#[test]
fn round_trips_dos_and_extended_timestamps() {
    let modified = ZipDateTime::new(2026, 11, 7, 12, 34, 56).unwrap().to_dos().unwrap();
    let mut entry = ZipEntry::stored("when.txt", b"x".to_vec());
    entry.modified = modified;
    entry.modified_unix = Some(1_791_398_400);
    let mut writer = ZipWriter::new();
    writer.push(entry);

    let index = index_zip_bytes("time.zip", writer.into_bytes().expect("write")).expect("index");
    assert_eq!(index.members[0].modified, modified);
    assert_eq!(index.members[0].modified.parts(), Some(ZipDateTime::new(2026, 11, 7, 12, 34, 56).unwrap()));
    assert_eq!(index.members[0].modified_unix, Some(1_791_398_400));
}

#[test]
fn writes_zip64_records_when_forced() {
    let payload = b"zip64 payload".to_vec();
    let mut writer = ZipWriter::with_zip64(Zip64Mode::Always);
    writer.push(ZipEntry::stored("first.txt", payload.clone()));
    writer.push(ZipEntry::deflated("folder/second.txt", payload.clone()));
    let bytes = writer.into_bytes().expect("write");

    let index = index_zip_bytes("zip64.zip", bytes.clone()).expect("index");
    assert!(index.eocd.zip64);
    assert_eq!(index.members.len(), 2);
    assert_eq!(index.members[1].uncompressed_size, payload.len() as u64);
    assert_eq!(index.members[1].raw_name, b"folder/second.txt".to_vec());

    let source = MemorySource::from_bytes("zip64.zip", bytes);
    let decoded = read_member_payload(source, &index.members[1], &mut BudgetTracker::new(Budget::default())).expect("decode");
    assert_eq!(decoded, payload);
}

#[test]
fn small_archive_does_not_emit_zip64_by_default() {
    let mut writer = ZipWriter::new();
    writer.push(ZipEntry::stored("a", b"x".to_vec()));
    let index = index_zip_bytes("plain.zip", writer.into_bytes().expect("write")).expect("index");
    assert!(!index.eocd.zip64);
    assert_eq!(index.members[0].compression_method, 0);
}

#[test]
fn writes_directory_markers_and_external_attributes() {
    let modified = ZipDateTime::new(2020, 1, 2, 3, 4, 6).unwrap().to_dos().unwrap();
    let mut writer = ZipWriter::new();
    writer.push(ZipEntry::directory("folder/", modified));
    let index = index_zip_bytes("dir.zip", writer.into_bytes().expect("write")).expect("index");

    assert!(index.members[0].name_is_directory());
    assert!(index.members[0].attributes_are_directory());
    assert!(index.members[0].is_directory());
    assert_eq!(index.members[0].modified, modified);
}

#[test]
fn rejects_an_unhonest_utf8_flag() {
    let mut entry = ZipEntry::stored([0x82u8], b"x".to_vec());
    entry.name_is_utf8 = true;
    let mut writer = ZipWriter::new();
    writer.push(entry);
    assert_eq!(writer.into_bytes(), Err(ZipWriteError::InvalidUtf8Name));
}

#[test]
fn rejects_extended_timestamp_outside_32_bits() {
    let mut entry = ZipEntry::stored("a", b"x".to_vec());
    entry.modified_unix = Some(i64::from(i32::MAX) + 1);
    let mut writer = ZipWriter::new();
    writer.push(entry);
    assert_eq!(writer.into_bytes(), Err(ZipWriteError::ExtendedTimestampOutOfRange { value: i64::from(i32::MAX) + 1 }));
}

#[test]
fn rejects_names_over_the_16_bit_field() {
    let long_name = vec![b'a'; u16::MAX as usize + 1];
    let mut writer = ZipWriter::new();
    writer.push(ZipEntry::stored(long_name, b"x".to_vec()));
    assert_eq!(writer.into_bytes(), Err(ZipWriteError::NameTooLong { length: u16::MAX as usize + 1 }));
}

#[test]
fn rejects_odd_dos_seconds_in_components() {
    assert_eq!(ZipDateTime::new(2026, 11, 7, 12, 34, 55), None);
    assert_eq!(ZipDosDateTime::ZERO.parts(), None);
}

#[test]
fn compression_defaults_to_store() {
    assert_eq!(ZipCompression::default(), ZipCompression::Store);
}
