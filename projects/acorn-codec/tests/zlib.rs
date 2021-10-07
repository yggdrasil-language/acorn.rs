use acorn_codec::{
    ChecksumAlgorithm, CodecError, CodecLimits,
    compression::{StreamCodec, Zlib, deflate::DeflateLevel},
};

#[test]
fn zlib_stored_round_trip() {
    let payload = b"acorn-codec zlib";
    let compressed = Zlib::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let facts = Zlib::decode(&compressed, limits).expect("decode");
    assert_eq!(facts.value, payload);
    assert!(facts.fully_consumed(compressed.len()));
}

#[test]
fn zlib_default_level_round_trip() {
    let payload = b"the quick brown fox jumps over the lazy dog. ".repeat(40);
    let compressed = Zlib::encode(&payload, DeflateLevel::Default);
    assert!(compressed.len() < payload.len());
    let facts = Zlib::decode(&compressed, CodecLimits::new().with_max_output_bytes(1 << 20)).expect("decode");
    assert_eq!(facts.value, payload);
}

#[test]
fn zlib_stream_decoder_accepts_chunks() {
    let payload = b"acorn-codec zlib stream";
    let compressed = Zlib::encode(payload, DeflateLevel::Store);
    let mut decoder = <Zlib as StreamCodec>::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed[..3]).expect("first chunk");
    decoder.push(&compressed[3..]).expect("second chunk");
    decoder.finish(&mut output).expect("finish");
    assert_eq!(output, payload.to_vec());

    let facts = decoder.facts().expect("facts");
    assert!(facts.complete);
    assert_eq!(facts.trailing_input, 0);
    assert_eq!(facts.output_produced as usize, payload.len());
}

#[test]
fn zlib_stream_decoder_reports_broken_trailer() {
    let mut compressed = Zlib::encode(b"acorn-codec zlib", DeflateLevel::Store);
    let last = compressed.len() - 1;
    compressed[last] ^= 0xFF;
    let mut decoder = Zlib::decoder(CodecLimits::new());
    let mut output = Vec::new();
    assert!(decoder.finish(&mut output).is_err());
    assert!(decoder.facts().is_none());
}

#[test]
fn zlib_rejects_a_preset_dictionary() {
    // CMF 0x78 needs FLG ≡ 1 (mod 31); 0x20 sets FDICT and keeps the header check valid.
    let input = [0x78u8, 0x20, 0, 0, 0, 0];
    let error = Zlib::decode(&input, CodecLimits::new()).expect_err("preset dictionary");
    assert!(matches!(error, CodecError::UnsupportedFeature { .. }));
}

#[test]
fn zlib_rejects_an_out_of_range_window_declaration() {
    // CMF 0x88 declares CINFO 8, a 64 KiB window Deflate cannot honour; FLG 28 keeps the check valid.
    let input = [0x88u8, 28, 0, 0, 0, 0];
    let error = Zlib::decode(&input, CodecLimits::new()).expect_err("window declaration");
    assert!(matches!(error, CodecError::InvalidEnvelope { .. }));
}

#[test]
fn zlib_reports_the_verified_adler32_in_stream_facts() {
    let payload = b"acorn-codec zlib checksum facts";
    let compressed = Zlib::encode(payload, DeflateLevel::Store);
    let mut decoder = Zlib::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed).expect("chunk");
    decoder.finish(&mut output).expect("finish");
    let checksum = decoder.facts().expect("facts").checksum.expect("checksum facts");
    assert_eq!(checksum.algorithm, ChecksumAlgorithm::Adler32);
    assert!(checksum.verified);
}

#[test]
fn deflate_stream_facts_carry_no_checksum() {
    let payload = b"acorn-codec deflate has no trailer";
    let compressed = acorn_codec::compression::Deflate::encode(payload, DeflateLevel::Store);
    let mut decoder = acorn_codec::compression::Deflate::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed).expect("chunk");
    decoder.finish(&mut output).expect("finish");
    assert_eq!(output, payload.to_vec());
    assert!(decoder.facts().expect("facts").checksum.is_none());
}
