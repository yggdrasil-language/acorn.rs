use acorn_codec::compression::envelope::zlib::encode_stored;
use acorn_core::Budget;
use acorn_source::{ByteSource, ReadOutcome};
use acorn_zlib::{ZLIB_CM_DEFLATE, decode_zlib_with_facts, index_zlib_bytes};

#[test]
fn decodes_zlib_payload_with_facts() {
    let bytes = encode_stored(b"hello zlib");
    let index = index_zlib_bytes(bytes.clone()).expect("index");
    assert_eq!(index.header.compression_method, ZLIB_CM_DEFLATE);
    let (decoded, facts) = decode_zlib_with_facts("sample.zlib", &bytes, &Budget::default()).expect("decode");
    let length = decoded.length().expect("length") as usize;
    let mut buffer = vec![0u8; length];
    assert_eq!(decoded.read_at(0, &mut buffer).expect("read"), ReadOutcome::Complete);
    assert_eq!(buffer, b"hello zlib");
    assert_eq!(facts.decoded_size as usize, buffer.len());
    assert!(facts.fully_consumed);
    assert_eq!(facts.header_size, index.header.header_size);
}

#[test]
fn rejects_non_zlib_bytes() {
    let err = index_zlib_bytes(vec![0x1F, 0x8B, 0x08, 0x00]).expect_err("not zlib");
    assert_eq!(err.to_string(), "zlib signature not found");
}
