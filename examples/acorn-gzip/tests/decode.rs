use acorn_codec::compression::envelope::gzip::encode_stored;
use acorn_core::Budget;
use acorn_gzip::{decode_gzip_with_facts, index_gzip_bytes};
use acorn_source::{ByteSource, ReadOutcome};

#[test]
fn decodes_gzip_payload_with_facts() {
    let bytes = encode_stored(b"hello gzip");
    let index = index_gzip_bytes(bytes.clone()).expect("index");
    assert_eq!(index.header.compression_method, acorn_gzip::GZIP_CM_DEFLATE);
    let (decoded, facts) = decode_gzip_with_facts("sample.gz", &bytes, &Budget::default()).expect("decode");
    let length = decoded.length().expect("length") as usize;
    let mut buffer = vec![0u8; length];
    assert_eq!(decoded.read_at(0, &mut buffer).expect("read"), ReadOutcome::Complete);
    assert_eq!(buffer, b"hello gzip");
    assert_eq!(facts.decoded_size as usize, buffer.len());
    assert!(facts.fully_consumed);
    assert_eq!(facts.header_size, index.header.header_size);
}

#[test]
fn rejects_non_gzip_bytes() {
    let err = index_gzip_bytes(vec![0x50, 0x4B, 0x03, 0x04]).expect_err("not gzip");
    assert_eq!(err.to_string(), "gzip signature not found");
}
