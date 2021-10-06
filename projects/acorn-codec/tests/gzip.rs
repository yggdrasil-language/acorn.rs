use acorn_codec::{
    CodecLimits,
    compression::{Gzip, StreamCodec, envelope::gzip::encode_stored},
};

#[test]
fn gzip_stored_round_trip() {
    let payload = b"acorn-codec gzip";
    let compressed = encode_stored(payload);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let facts = Gzip::decode(&compressed, limits).expect("decode");
    assert_eq!(facts.value, payload);
    assert!(facts.fully_consumed(compressed.len()));
}

#[test]
fn gzip_stream_decoder_accepts_chunks() {
    let payload = b"acorn-codec gzip stream";
    let compressed = encode_stored(payload);
    let mut decoder = <Gzip as StreamCodec>::decoder(CodecLimits::new().with_max_output_bytes(1024));
    let mut output = Vec::new();
    decoder.push(&compressed[..5]);
    decoder.push(&compressed[5..]);
    decoder.finish(&mut output).expect("finish");
    assert_eq!(output, payload.to_vec());

    let facts = decoder.facts().expect("facts");
    assert!(facts.complete);
    assert_eq!(facts.trailing_input, 0);
}
