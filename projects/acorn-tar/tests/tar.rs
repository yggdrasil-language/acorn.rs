use acorn_codec::CodecLimits;
use acorn_core::{Budget, BudgetTracker};
use acorn_source::MemorySource;
use acorn_tar::{
    TarEntryDraft, TarEntryKind, TarGz, TarIndexError, TarWriteError, TarWriter, index_tar_bytes, read_member_payload,
    write_tar,
};

#[test]
fn round_trips_files_directories_and_links() {
    let mut file = TarEntryDraft::file("docs/readme.txt", b"hello tar".to_vec());
    file.modified_unix = 1_700_000_000;
    file.mode = 0o644;
    let bytes = write_tar(vec![
        TarEntryDraft::directory("docs/"),
        file,
        TarEntryDraft::symlink("docs/latest.txt", "readme.txt"),
        TarEntryDraft::hard_link("docs/copy.txt", "docs/readme.txt"),
    ])
    .expect("write");

    let source_bytes = bytes.clone();
    let archive = index_tar_bytes("sample.tar", bytes).expect("index");
    assert_eq!(archive.entries.len(), 4);
    assert_eq!(archive.entries[0].path, b"docs/".to_vec());
    assert_eq!(archive.entries[0].kind, TarEntryKind::Directory);
    assert_eq!(archive.entries[1].path, b"docs/readme.txt".to_vec());
    assert_eq!(archive.entries[1].kind, TarEntryKind::Regular);
    assert_eq!(archive.entries[1].modified_unix, 1_700_000_000);
    assert_eq!(archive.entries[1].size, 9);
    assert_eq!(archive.entries[2].kind, TarEntryKind::Symlink);
    assert_eq!(archive.entries[2].link_name, b"readme.txt".to_vec());
    assert_eq!(archive.entries[3].kind, TarEntryKind::HardLink);

    let payload = read_member_payload(
        MemorySource::from_bytes("sample.tar", source_bytes),
        &archive.entries[1],
        &mut BudgetTracker::new(Budget::default()),
    )
    .expect("payload");
    assert_eq!(payload, b"hello tar");
}

#[test]
fn uses_ustar_prefix_for_names_up_to_255_bytes() {
    let path = format!("{}/{}", "a".repeat(120), "b".repeat(120));
    let bytes = write_tar(vec![TarEntryDraft::file(&path, b"x".to_vec())]).expect("write");
    let archive = index_tar_bytes("prefix.tar", bytes).expect("index");
    assert_eq!(archive.entries[0].path, path.as_bytes().to_vec());
}

#[test]
fn writes_gnu_long_names_and_link_targets() {
    let path = format!("{}/{}", "a".repeat(250), "file.txt");
    let target = "target/".repeat(40);
    let bytes = write_tar(vec![TarEntryDraft::symlink(&path, &target)]).expect("write");
    let archive = index_tar_bytes("long.tar", bytes).expect("index");

    assert_eq!(archive.entries[0].path, path.as_bytes().to_vec());
    assert_eq!(archive.entries[0].link_name, target.as_bytes().to_vec());
    assert_eq!(archive.entries[0].kind, TarEntryKind::Symlink);
}

#[test]
fn preserves_raw_non_utf8_path_bytes() {
    let path = [0x82, b'/', b'f', b'i', b'l', b'e'];
    let bytes = write_tar(vec![TarEntryDraft::file(path, Vec::new())]).expect("write");
    let archive = index_tar_bytes("raw.tar", bytes).expect("index");
    assert_eq!(archive.entries[0].path, path);
}

#[test]
fn rejects_data_on_a_directory() {
    let mut entry = TarEntryDraft::directory("dir/");
    entry.data = b"payload".to_vec();
    let mut writer = TarWriter::new();
    writer.push(entry);
    assert_eq!(writer.into_bytes(), Err(TarWriteError::DataOnNonRegularEntry { kind: TarEntryKind::Directory }));
}

#[test]
fn rejects_a_link_name_on_a_regular_file() {
    let mut entry = TarEntryDraft::file("file.txt", Vec::new());
    entry.link_name = b"target".to_vec();
    let mut writer = TarWriter::new();
    writer.push(entry);
    assert_eq!(writer.into_bytes(), Err(TarWriteError::LinkNameOnNonLinkEntry { kind: TarEntryKind::Regular }));
}

#[test]
fn detects_a_bad_header_checksum() {
    let mut bytes = write_tar(vec![TarEntryDraft::file("a.txt", b"payload".to_vec())]).expect("write");
    bytes[10] ^= 0x01;
    let error = index_tar_bytes("bad.tar", bytes).expect_err("checksum");
    assert!(matches!(error, TarIndexError::ChecksumMismatch { .. }), "{error:?}");
}

#[test]
fn empty_archive_indexes_without_members() {
    let bytes = write_tar(Vec::new()).expect("write");
    assert_eq!(bytes.len(), 1024);
    let archive = index_tar_bytes("empty.tar", bytes).expect("index");
    assert!(archive.entries.is_empty());
    assert_eq!(archive.end_of_archive, 0);
}

#[test]
fn compose_tar_gzip_round_trip() {
    let tar = write_tar(vec![
        TarEntryDraft::file("a.txt", b"alpha".to_vec()),
        TarEntryDraft::file("b.txt", "bravo ".repeat(50).into_bytes()),
    ])
    .expect("tar");
    let encoded = TarGz::encode(&tar);
    let decoded = TarGz::decode("sample.tar.gz", &encoded, CodecLimits::new()).expect("gunzip");
    assert_eq!(decoded.tar_bytes(), tar.as_slice());
    assert_eq!(decoded.archive().entries.len(), 2);
    assert_eq!(decoded.payload(&decoded.archive().entries[0]), Some(b"alpha".as_slice()));
    assert_eq!(decoded.archive().entries[1].size, ("bravo ".repeat(50).len()) as u64);
}
