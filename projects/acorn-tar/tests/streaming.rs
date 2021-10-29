use acorn_codec::{CodecError, CodecLimits};
use acorn_core::Budget;
use acorn_source::MemorySource;
use acorn_tar::{
    StreamChunk, TarEntryDraft, TarEntryStream, TarGzError, TarGzStream, TarWriter, index_tar_bytes, read_member_payload,
};

fn sample_tar() -> Vec<u8> {
    let mut writer = TarWriter::new();
    writer.push(TarEntryDraft::directory("folder/"));
    writer.push(TarEntryDraft::file("folder/one.txt", "first member body ".repeat(400).into_bytes()));
    writer.push(TarEntryDraft::file("folder/second.txt", "second member body"));
    writer.push(TarEntryDraft::symlink("folder/link", "one.txt"));
    writer.into_bytes().expect("tar bytes")
}

fn drain_tar<S: acorn_source::ByteSource>(mut stream: TarEntryStream<S>, out_size: usize) -> (Vec<u8>, u64) {
    let mut collected = Vec::new();
    let mut buffer = vec![0u8; out_size];
    loop {
        match stream.read(&mut buffer).expect("read") {
            StreamChunk::Produced(count) => collected.extend_from_slice(&buffer[..count]),
            StreamChunk::Finished => break,
            StreamChunk::NeedInput => panic!("tar payload reads never need input"),
        }
    }
    (collected, stream.read_bytes())
}

#[test]
fn entry_stream_matches_one_shot_payloads() {
    let bytes = sample_tar();
    let archive = index_tar_bytes("sample.tar", bytes.clone()).expect("index");
    for entry in &archive.entries {
        if entry.data_range.len == 0 {
            continue;
        }
        let expected = read_member_payload(
            MemorySource::from_bytes("sample.tar", bytes.clone()),
            entry,
            &mut acorn_core::BudgetTracker::new(Budget::default()),
        )
        .expect("one-shot payload");
        let stream = TarEntryStream::open(MemorySource::from_bytes("sample.tar", bytes.clone()), entry).expect("open");
        let (collected, read) = drain_tar(stream, 37);
        assert_eq!(collected, expected, "entry {}", String::from_utf8_lossy(&entry.path));
        assert_eq!(read, entry.size);
    }
}

#[test]
fn entry_stream_handles_an_empty_payload() {
    let bytes = sample_tar();
    let archive = index_tar_bytes("sample.tar", bytes.clone()).expect("index");
    let directory = archive.entries.iter().find(|entry| entry.data_range.len == 0).expect("directory entry");
    let mut stream = TarEntryStream::open(MemorySource::from_bytes("sample.tar", bytes), directory).expect("open");
    let mut buffer = [0u8; 8];
    assert_eq!(stream.read(&mut buffer), Ok(StreamChunk::Finished));
    assert_eq!(stream.read_bytes(), 0);
}

fn drain_gz_entry(entry: &mut acorn_tar::TarGzEntry<'_, MemorySource>, out_size: usize) -> Result<Vec<u8>, TarGzError> {
    let mut collected = Vec::new();
    let mut buffer = vec![0u8; out_size];
    loop {
        match entry.read(&mut buffer)? {
            StreamChunk::Produced(count) => collected.extend_from_slice(&buffer[..count]),
            StreamChunk::Finished => return Ok(collected),
            StreamChunk::NeedInput => panic!("tar-gzip payload reads never need input"),
        }
    }
}

#[test]
fn tar_gz_stream_matches_the_index() {
    let tar = sample_tar();
    let archive = index_tar_bytes("sample.tar", tar.clone()).expect("index");
    let gz = acorn_tar::TarGz::encode(&tar);

    let mut stream = TarGzStream::open(MemorySource::from_bytes("sample.tar.gz", gz), CodecLimits::new()).expect("open");
    let mut seen = 0usize;
    while let Some(mut entry) = stream.next_entry().expect("next entry") {
        let indexed = &archive.entries[seen];
        assert_eq!(entry.entry().path, indexed.path);
        assert_eq!(entry.entry().kind, indexed.kind);
        assert_eq!(entry.entry().modified_unix, indexed.modified_unix);
        let payload = drain_gz_entry(&mut entry, 29).expect("payload");
        assert_eq!(payload.len() as u64, indexed.size, "member {}", String::from_utf8_lossy(&indexed.path));
        if indexed.data_range.len != 0 {
            let expected = read_member_payload(
                MemorySource::from_bytes("sample.tar", tar.clone()),
                indexed,
                &mut acorn_core::BudgetTracker::new(Budget::default()),
            )
            .expect("one-shot payload");
            assert_eq!(payload, expected);
        }
        seen += 1;
    }
    assert_eq!(seen, archive.entries.len());
    assert!(stream.is_finished());
}

#[test]
fn tar_gz_stream_skips_an_unconsumed_payload() {
    let gz = acorn_tar::TarGz::encode(&sample_tar());
    let mut stream =
        TarGzStream::with_chunk_size(MemorySource::from_bytes("sample.tar.gz", gz), CodecLimits::new(), 33).expect("open");

    let directory = stream.next_entry().expect("first").expect("directory");
    assert_eq!(directory.entry().kind, acorn_tar::TarEntryKind::Directory);
    drop(directory);

    let mut first = stream.next_entry().expect("second").expect("entry");
    assert_eq!(first.entry().path, b"folder/one.txt".to_vec());
    let mut buffer = [0u8; 5];
    assert!(matches!(first.read(&mut buffer), Ok(StreamChunk::Produced(5))));
    drop(first);

    let mut second = stream.next_entry().expect("third").expect("entry");
    assert_eq!(second.entry().path, b"folder/second.txt".to_vec());
    let payload = drain_gz_entry(&mut second, 6).expect("payload");
    assert_eq!(payload, b"second member body");
}

#[test]
fn tar_gz_stream_reports_trailing_input() {
    let mut gz = acorn_tar::TarGz::encode(&sample_tar());
    gz.extend_from_slice(b"junk after the member");
    let mut stream = TarGzStream::open(MemorySource::from_bytes("sample.tar.gz", gz), CodecLimits::new()).expect("open");
    let mut seen = 0usize;
    let error = loop {
        match stream.next_entry() {
            Ok(Some(mut entry)) => {
                drain_gz_entry(&mut entry, 32).expect("payload");
                seen += 1;
            }
            Ok(None) => panic!("trailing input must be reported, saw {seen} entries"),
            Err(error) => break error,
        }
    };
    assert!(matches!(error, TarGzError::TrailingInput { .. }));
}

#[test]
fn tar_gz_stream_enforces_decoding_limits() {
    let gz = acorn_tar::TarGz::encode(&sample_tar());
    let limits = CodecLimits::new().with_max_output_bytes(64);
    let mut stream = TarGzStream::open(MemorySource::from_bytes("sample.tar.gz", gz), limits).expect("open");
    let error = loop {
        match stream.next_entry() {
            Ok(Some(mut entry)) => {
                if let Err(error) = drain_gz_entry(&mut entry, 32) {
                    break error;
                }
            }
            Ok(None) => panic!("the decoding limit must fire"),
            Err(error) => break error,
        }
    };
    assert!(matches!(error, TarGzError::Codec(CodecError::OutputLimitExceeded { .. })));
}

#[test]
fn tar_gz_stream_reports_truncation() {
    let mut gz = acorn_tar::TarGz::encode(&sample_tar());
    gz.truncate(gz.len() / 3);
    let mut stream = TarGzStream::open(MemorySource::from_bytes("sample.tar.gz", gz), CodecLimits::new()).expect("open");
    let error = loop {
        match stream.next_entry() {
            Ok(Some(mut entry)) => {
                if let Err(error) = drain_gz_entry(&mut entry, 32) {
                    break error;
                }
            }
            Err(error) => break error,
            Ok(None) => panic!("a truncated archive must not finish"),
        }
    };
    assert!(matches!(error, TarGzError::Codec(CodecError::InputExhausted { .. }) | TarGzError::Tar(_)));
}

#[test]
fn tar_gz_stream_reads_gnu_long_names() {
    let path = format!("folder/{}.txt", "long-segment-".repeat(12));
    let mut writer = TarWriter::new();
    writer.push(TarEntryDraft::file(path.clone(), b"long name body".to_vec()));
    writer.push(TarEntryDraft::file("short.txt", b"short body".to_vec()));
    let tar = writer.into_bytes().expect("tar");
    let gz = acorn_tar::TarGz::encode(&tar);

    let mut stream = TarGzStream::open(MemorySource::from_bytes("long.tar.gz", gz), CodecLimits::new()).expect("open");
    let mut first = stream.next_entry().expect("first").expect("entry");
    assert_eq!(first.entry().path, path.as_bytes().to_vec());
    assert_eq!(drain_gz_entry(&mut first, 9).expect("payload"), b"long name body");
    drop(first);

    let mut second = stream.next_entry().expect("second").expect("entry");
    assert_eq!(second.entry().path, b"short.txt".to_vec());
    assert_eq!(drain_gz_entry(&mut second, 4).expect("payload"), b"short body");
}

#[test]
fn tar_gz_stream_handles_an_empty_archive() {
    let tar = acorn_tar::write_tar(Vec::new()).expect("empty tar");
    let gz = acorn_tar::TarGz::encode(&tar);
    let mut stream = TarGzStream::open(MemorySource::from_bytes("empty.tar.gz", gz), CodecLimits::new()).expect("open");
    assert!(stream.next_entry().expect("next").is_none());
    assert!(stream.is_finished());
    assert!(stream.next_entry().expect("idempotent").is_none());
}
