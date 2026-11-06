use acorn_core::{Budget, BudgetTracker};
use acorn_source::{MemorySink, MemorySource};
use acorn_zip::{
    FLAG_DATA_DESCRIPTOR, Zip64Mode, ZipCompression, ZipDateTime, ZipDosDateTime, ZipStreamEntry, ZipStreamWriter,
    ZipWriteError, index_zip_bytes, read_member_payload,
};

fn chunked_archive(compression: ZipCompression, payload: &[u8], chunk: usize) -> Vec<u8> {
    let entry = match compression {
        ZipCompression::Store => ZipStreamEntry::stored("member.bin"),
        ZipCompression::Deflate => ZipStreamEntry::deflated("member.bin"),
    };
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    writer.begin_entry(entry).expect("begin");
    for part in payload.chunks(chunk) {
        writer.write_chunk(part).expect("chunk");
    }
    writer.finish_entry().expect("finish");
    writer.close().expect("close");
    writer.into_sink().into_bytes()
}

#[test]
fn streams_stored_and_deflated_members() {
    let payload = "streamed payload ".repeat(700).into_bytes();
    let cases = [(ZipCompression::Store, 0u16), (ZipCompression::Deflate, 8)];
    for (compression, method) in cases {
        let bytes = chunked_archive(compression, &payload, 257);
        let index = index_zip_bytes("stream.zip", bytes.clone()).expect("index");
        assert_eq!(index.members.len(), 1);
        assert_eq!(index.members[0].compression_method, method);
        assert_eq!(index.members[0].uncompressed_size, payload.len() as u64);
        assert_ne!(index.members[0].general_purpose_bit_flag & FLAG_DATA_DESCRIPTOR, 0, "descriptor flag");

        let decoded = read_member_payload(
            MemorySource::from_bytes("stream.zip", bytes),
            &index.members[0],
            &mut BudgetTracker::new(Budget::default()),
        )
        .expect("decode");
        assert_eq!(decoded, payload);
        if method == 8 {
            assert!(index.members[0].compressed_size < payload.len() as u64, "deflate must shrink this payload");
        }
    }
}

#[test]
fn streams_several_members_with_metadata() {
    let modified = ZipDateTime::new(2026, 11, 7, 12, 34, 56).unwrap().to_dos().unwrap();
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    writer.begin_entry(ZipStreamEntry::directory("folder/", modified)).expect("directory");
    writer.finish_entry().expect("finish directory");

    let mut entry = ZipStreamEntry::deflated("folder/second.txt");
    entry.modified = modified;
    entry.modified_unix = Some(1_791_398_400);
    writer.begin_entry(entry).expect("begin");
    for part in b"second member body".chunks(4) {
        writer.write_chunk(part).expect("chunk");
    }
    writer.finish_entry().expect("finish");
    writer.close().expect("close");

    let bytes = writer.into_sink().into_bytes();
    let index = index_zip_bytes("multi.zip", bytes).expect("index");
    assert_eq!(index.members.len(), 2);
    assert!(index.members[0].is_directory());
    assert_eq!(index.members[0].modified, modified);
    assert_eq!(index.members[1].raw_name, b"folder/second.txt".to_vec());
    assert_eq!(index.members[1].modified_unix, Some(1_791_398_400));
    assert_eq!(index.members[1].uncompressed_size, 18);
}

#[test]
fn forced_zip64_streaming_round_trips() {
    let payload = b"zip64 streamed".to_vec();
    let mut writer = ZipStreamWriter::with_zip64(MemorySink::new(), Zip64Mode::Always);
    writer.begin_entry(ZipStreamEntry::stored("a.bin")).expect("begin");
    writer.write_chunk(&payload).expect("chunk");
    writer.finish_entry().expect("finish");
    writer.close().expect("close");

    let bytes = writer.into_sink().into_bytes();
    let index = index_zip_bytes("zip64-stream.zip", bytes.clone()).expect("index");
    assert!(index.eocd.zip64);
    assert_eq!(index.members[0].uncompressed_size, payload.len() as u64);

    let decoded = read_member_payload(
        MemorySource::from_bytes("zip64-stream.zip", bytes),
        &index.members[0],
        &mut BudgetTracker::new(Budget::default()),
    )
    .expect("decode");
    assert_eq!(decoded, payload);
}

#[test]
fn small_streamed_archive_uses_classic_central_fields() {
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    writer.begin_entry(ZipStreamEntry::stored("a")).expect("begin");
    writer.write_chunk(b"x").expect("chunk");
    writer.finish_entry().expect("finish");
    writer.close().expect("close");

    let index = index_zip_bytes("small-stream.zip", writer.into_sink().into_bytes()).expect("index");
    assert!(!index.eocd.zip64);
    assert_eq!(index.members[0].compression_method, 0);
}

#[test]
fn reports_misuse_without_writing_a_broken_archive() {
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    assert_eq!(writer.write_chunk(b"x"), Err(ZipWriteError::NoEntryInProgress));
    assert_eq!(writer.finish_entry(), Err(ZipWriteError::NoEntryInProgress));

    writer.begin_entry(ZipStreamEntry::stored("a")).expect("begin");
    assert_eq!(writer.begin_entry(ZipStreamEntry::stored("b")), Err(ZipWriteError::EntryInProgress));
    assert_eq!(writer.close(), Err(ZipWriteError::EntryInProgress));

    writer.finish_entry().expect("finish");
    writer.close().expect("close");
    assert_eq!(writer.begin_entry(ZipStreamEntry::stored("c")), Err(ZipWriteError::Closed));
}

#[test]
fn closed_writer_rejects_further_writes() {
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    writer.close().expect("close empty archive");
    assert_eq!(writer.close(), Err(ZipWriteError::Closed));
    assert_eq!(writer.begin_entry(ZipStreamEntry::stored("a")), Err(ZipWriteError::Closed));
}

#[test]
fn empty_archive_indexes_without_members() {
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    writer.close().expect("close");
    let index = index_zip_bytes("empty-stream.zip", writer.into_sink().into_bytes()).expect("index");
    assert!(index.members.is_empty());
}

#[test]
fn rejects_an_unhonest_utf8_name() {
    let mut entry = ZipStreamEntry::stored([0x82u8]);
    entry.name_is_utf8 = true;
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    assert_eq!(writer.begin_entry(entry), Err(ZipWriteError::InvalidUtf8Name));
}

#[test]
fn zero_length_member_round_trips() {
    let mut writer = ZipStreamWriter::new(MemorySink::new());
    writer.begin_entry(ZipStreamEntry::deflated("empty")).expect("begin");
    writer.finish_entry().expect("finish");
    writer.close().expect("close");

    let bytes = writer.into_sink().into_bytes();
    let index = index_zip_bytes("empty-member.zip", bytes.clone()).expect("index");
    assert_eq!(index.members[0].uncompressed_size, 0);
    let decoded = read_member_payload(
        MemorySource::from_bytes("empty-member.zip", bytes),
        &index.members[0],
        &mut BudgetTracker::new(Budget::default()),
    )
    .expect("decode empty member");
    assert!(decoded.is_empty());
}

#[test]
fn directory_marker_defaults_are_consistent() {
    assert_eq!(ZipDosDateTime::ZERO.parts(), None);
}
