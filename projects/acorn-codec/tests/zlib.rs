use acorn_codec::{
    CodecLimits,
    compression::{StreamCodec, Zlib, envelope::zlib::encode_stored},
};

#[test]
fn zlib_stored_round_trip() {
    let payload = b"acorn-codec zlib";
    let compressed = encode_stored(payload);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let facts = Zlib::decode(&compressed, limits).expect("decode");
    assert_eq!(facts.value, payload);
    assert!(facts.fully_consumed(compressed.len()));
}

#[test]
fn zlib_stream_decoder_accepts_chunks() {
    let payload = b"acorn-codec zlib stream";
    let compressed = encode_stored(payload);
    let mut decoder = <Zlib as StreamCodec>::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed[..3]);
    decoder.push(&compressed[3..]);
    decoder.finish(&mut output).expect("finish");
    assert_eq!(output, payload.to_vec());

    let facts = decoder.facts().expect("facts");
    assert!(facts.complete);
    assert_eq!(facts.trailing_input, 0);
    assert_eq!(facts.output_produced as usize, payload.len());
}

#[test]
fn zlib_stream_decoder_reports_broken_trailer() {
    let mut compressed = encode_stored(b"acorn-codec zlib");
    let last = compressed.len() - 1;
    compressed[last] ^= 0xFF;
    let mut decoder = Zlib::decoder(CodecLimits::new());
    let mut output = Vec::new();
    assert!(decoder.finish(&mut output).is_err());
    assert!(decoder.facts().is_none());
}
