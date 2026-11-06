use acorn_codec::{CodecLimits, compression::StreamChunk};
use acorn_source::{ByteSource, MemorySource};
use acorn_zip::{
    ZipCompression, ZipMember, ZipMemberStream, ZipStreamEntry, ZipStreamWriter, ZipViewError, index_zip_bytes,
    read_member_payload,
};

fn build_archive(compression: ZipCompression, payload: &[u8]) -> (Vec<u8>, ZipMember) {
    let entry = match compression {
        ZipCompression::Store => ZipStreamEntry::stored("payload.bin"),
        ZipCompression::Deflate => ZipStreamEntry::deflated("payload.bin"),
    };
    let mut writer = ZipStreamWriter::new(acorn_source::MemorySink::new());
    writer.begin_entry(entry).expect("begin");
    for part in payload.chunks(311) {
        writer.write_chunk(part).expect("chunk");
    }
    writer.finish_entry().expect("finish");
    writer.close().expect("close");
    let bytes = writer.into_sink().into_bytes();
    let index = index_zip_bytes("stream.zip", bytes.clone()).expect("index");
    (bytes, index.members[0].clone())
}

fn drain<S: ByteSource>(mut stream: ZipMemberStream<S>, out_size: usize) -> (Vec<u8>, u64) {
    let mut collected = Vec::new();
    let mut buffer = vec![0u8; out_size];
    loop {
        match stream.read(&mut buffer).expect("read") {
            StreamChunk::Produced(count) => collected.extend_from_slice(&buffer[..count]),
            StreamChunk::Finished => break,
            StreamChunk::NeedInput => panic!("the reader resolves input needs itself"),
        }
    }
    (collected, stream.decoded_bytes())
}

#[test]
fn streams_stored_payload_in_caller_chunks() {
    let payload = "stored streaming body ".repeat(500).into_bytes();
    let (bytes, member) = build_archive(ZipCompression::Store, &payload);
    let stream =
        ZipMemberStream::with_chunk_size(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new(), 17)
            .expect("open");
    let (collected, decoded) = drain(stream, 23);
    assert_eq!(collected, payload);
    assert_eq!(decoded, payload.len() as u64);
}

#[test]
fn streams_deflated_payload_in_caller_chunks() {
    let payload = "deflated streaming body ".repeat(900).into_bytes();
    let (bytes, member) = build_archive(ZipCompression::Deflate, &payload);
    let stream =
        ZipMemberStream::with_chunk_size(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new(), 71)
            .expect("open");
    let (collected, decoded) = drain(stream, 13);
    assert_eq!(collected, payload);
    assert_eq!(decoded, payload.len() as u64);
}

#[test]
fn decoded_byte_count_tracks_partial_reads() {
    let payload = b"partial reading keeps limits answerable".repeat(64);
    let (bytes, member) = build_archive(ZipCompression::Deflate, &payload);
    let mut stream =
        ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new()).expect("open");

    let mut buffer = vec![0u8; 100];
    let first = stream.read(&mut buffer).expect("first read");
    let produced = match first {
        StreamChunk::Produced(count) => count,
        other => panic!("expected output, got {other:?}"),
    };
    assert_eq!(stream.decoded_bytes(), produced as u64);
    assert!(!stream.is_finished());

    let (rest, total) = drain(stream, 7);
    assert_eq!(rest.len() + produced, payload.len());
    assert_eq!(total, payload.len() as u64);
}

#[test]
fn rejects_a_member_whose_crc_does_not_match() {
    let (bytes, mut member) = build_archive(ZipCompression::Store, b"crc mismatch");
    member.crc32 ^= 0xFFFF_FFFF;
    let mut stream =
        ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new()).expect("open");
    let mut collected = Vec::new();
    let mut buffer = vec![0u8; 8];
    let error = loop {
        match stream.read(&mut buffer) {
            Ok(StreamChunk::Produced(count)) => collected.extend_from_slice(&buffer[..count]),
            Ok(StreamChunk::Finished) => panic!("a bad CRC must not finish"),
            Ok(StreamChunk::NeedInput) => panic!("unexpected input need"),
            Err(error) => break error,
        }
    };
    assert!(matches!(error, ZipViewError::CrcMismatch { .. }));
}

#[test]
fn truncated_deflate_payload_reports_input_exhaustion() {
    let payload = b"truncated deflate body".repeat(40);
    let (bytes, mut member) = build_archive(ZipCompression::Deflate, &payload);
    member.compressed_size = 1;
    member.payload_range.len = 1;
    let mut stream =
        ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new()).expect("open");
    let mut buffer = vec![0u8; 8];
    let error = loop {
        match stream.read(&mut buffer) {
            Ok(StreamChunk::Produced(_)) => continue,
            Ok(other) => panic!("expected a failure, got {other:?}"),
            Err(error) => break error,
        }
    };
    assert!(matches!(error, ZipViewError::Codec(acorn_codec::CodecError::InputExhausted { .. })));
}

#[test]
fn output_limit_stops_decoding_immediately() {
    let payload = b"limit guarded payload".repeat(200);
    let (bytes, member) = build_archive(ZipCompression::Deflate, &payload);
    let limits = CodecLimits::new().with_max_output_bytes(64);
    let mut stream = ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, limits).expect("open");
    let mut buffer = vec![0u8; 32];
    let mut produced = 0u64;
    let error = loop {
        match stream.read(&mut buffer) {
            Ok(StreamChunk::Produced(count)) => produced += count as u64,
            Ok(StreamChunk::Finished) => panic!("limit must fire"),
            Ok(StreamChunk::NeedInput) => panic!("unexpected input need"),
            Err(error) => break error,
        }
    };
    assert!(matches!(error, ZipViewError::BudgetExceeded));
    assert!(produced <= 64, "decoding stopped near the limit, produced {produced}");
}

#[test]
fn empty_member_finishes_without_output() {
    let (bytes, member) = build_archive(ZipCompression::Deflate, b"");
    let mut stream =
        ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new()).expect("open");
    let mut buffer = [0u8; 16];
    assert_eq!(stream.read(&mut buffer), Ok(StreamChunk::Finished));
    assert_eq!(stream.decoded_bytes(), 0);
    assert_eq!(stream.read(&mut buffer), Ok(StreamChunk::Finished));
}

#[test]
fn dropping_a_partial_reader_cancels_without_state() {
    let payload = b"cancel me halfway through".repeat(300);
    let (bytes, member) = build_archive(ZipCompression::Deflate, &payload);
    {
        let mut stream =
            ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes.clone()), &member, CodecLimits::new())
                .expect("open");
        let mut buffer = vec![0u8; 64];
        assert!(matches!(stream.read(&mut buffer), Ok(StreamChunk::Produced(_))));
    }

    let stream =
        ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new()).expect("reopen");
    let (collected, decoded) = drain(stream, 64);
    assert_eq!(collected, payload);
    assert_eq!(decoded, payload.len() as u64);
}

#[test]
fn streaming_matches_the_one_shot_decoder() {
    let payload = b"same bytes either way ".repeat(700);
    let (bytes, member) = build_archive(ZipCompression::Deflate, &payload);
    let one_shot = read_member_payload(
        MemorySource::from_bytes("stream.zip", bytes.clone()),
        &member,
        &mut acorn_core::BudgetTracker::new(acorn_core::Budget::default()),
    )
    .expect("one-shot");
    let stream =
        ZipMemberStream::open(MemorySource::from_bytes("stream.zip", bytes), &member, CodecLimits::new()).expect("open");
    let (collected, _) = drain(stream, 19);
    assert_eq!(collected, one_shot);
}
