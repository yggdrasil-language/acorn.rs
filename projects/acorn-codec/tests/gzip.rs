use acorn_codec::{
    ChecksumAlgorithm, CodecError, CodecLimits,
    compression::{Gzip, StreamCodec, deflate::DeflateLevel},
};

#[test]
fn gzip_stored_round_trip() {
    let payload = b"acorn-codec gzip";
    let compressed = Gzip::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let facts = Gzip::decode(&compressed, limits).expect("decode");
    assert_eq!(facts.value, payload);
    assert!(facts.fully_consumed(compressed.len()));
}

#[test]
fn gzip_member_writer_round_trips() {
    let payload = b"the quick brown fox jumps over the lazy dog. ".repeat(40);
    let first = Gzip::encode(&payload, DeflateLevel::Default);
    let second = Gzip::encode(&payload, DeflateLevel::Default);
    assert_eq!(first, second, "gzip member output must be reproducible");
    assert!(first.len() < payload.len());

    let facts = Gzip::decode(&first, CodecLimits::new().with_max_output_bytes(1 << 20)).expect("decode");
    assert_eq!(facts.value, payload);
    assert!(facts.fully_consumed(first.len()));
}

#[test]
fn gzip_stream_decoder_accepts_chunks() {
    let payload = b"acorn-codec gzip stream";
    let compressed = Gzip::encode(payload, DeflateLevel::Store);
    let mut decoder = <Gzip as StreamCodec>::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed[..5]).expect("first chunk");
    decoder.push(&compressed[5..]).expect("second chunk");
    decoder.finish(&mut output).expect("finish");
    assert_eq!(output, payload.to_vec());

    let facts = decoder.facts().expect("facts");
    assert!(facts.complete);
    assert_eq!(facts.trailing_input, 0);
}

/// Test-local CRC32 so header CRC16 bytes can be built without a codec helper.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn member_header(flags: u8) -> Vec<u8> {
    vec![0x1F, 0x8B, 0x08, flags, 0, 0, 0, 0, 0, 0xFF]
}

#[test]
fn gzip_reports_the_verified_crc32_in_stream_facts() {
    let payload = b"acorn-codec gzip checksum facts";
    let compressed = Gzip::encode(payload, DeflateLevel::Store);
    let mut decoder = Gzip::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed).expect("chunk");
    decoder.finish(&mut output).expect("finish");
    let checksum = decoder.facts().expect("facts").checksum.expect("checksum facts");
    assert_eq!(checksum.algorithm, ChecksumAlgorithm::Crc32);
    assert_eq!(checksum.value, u64::from(crc32(payload)));
    assert!(checksum.verified);
}

#[test]
fn gzip_validates_a_present_header_crc() {
    let payload = b"acorn-codec gzip header crc";
    let encoded = Gzip::encode(payload, DeflateLevel::Store);
    let header = member_header(0x02);
    let mut member = header.clone();
    member.extend_from_slice(&(crc32(&header) as u16).to_le_bytes());
    member.extend_from_slice(&encoded[10..]);
    let facts = Gzip::decode(&member, CodecLimits::new().with_max_output_bytes(1024)).expect("decode");
    assert_eq!(facts.value, payload);

    let mut broken = member;
    broken[10] ^= 0xFF;
    let error = Gzip::decode(&broken, CodecLimits::new()).expect_err("header crc");
    assert!(matches!(error, CodecError::ChecksumMismatch { algorithm: ChecksumAlgorithm::Crc32, .. }));
}

#[test]
fn gzip_rejects_a_truncated_extra_field() {
    let mut input = member_header(0x04);
    input.extend_from_slice(&64u16.to_le_bytes());
    input.extend_from_slice(&[0u8; 4]);
    let error = Gzip::decode(&input, CodecLimits::new()).expect_err("truncated extra");
    assert!(matches!(error, CodecError::InputExhausted { .. }));
}

#[test]
fn gzip_rejects_an_unterminated_name() {
    let mut input = member_header(0x08);
    input.extend_from_slice(b"name-without-terminator");
    let error = Gzip::decode(&input, CodecLimits::new()).expect_err("unterminated name");
    assert!(matches!(error, CodecError::InputExhausted { .. }));
}

#[test]
fn gzip_reports_a_following_member_as_trailing_input() {
    let payload = b"acorn-codec gzip single member";
    let member = Gzip::encode(payload, DeflateLevel::Store);
    let mut concatenated = member.clone();
    concatenated.extend_from_slice(&member);
    let facts = Gzip::decode(&concatenated, CodecLimits::new().with_max_output_bytes(1024)).expect("decode");
    assert_eq!(facts.value, payload);
    assert_eq!(facts.trailing_input, member.len());
    assert!(!facts.fully_consumed(concatenated.len()));
}
