use acorn_codec::{
    CodecError, CodecLimits, StreamState,
    compression::{Deflate, StreamCodec, Zlib, deflate::DeflateLevel},
};

#[test]
fn push_stops_at_the_buffered_input_limit() {
    let payload = b"acorn-codec bounded stream";
    let compressed = Zlib::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_input_bytes(4);
    let mut decoder = Zlib::decoder(limits);
    decoder.push(&compressed[..4]).expect("chunk fits");

    let offered = (compressed.len() - 4) as u64;
    let error = decoder.push(&compressed[4..]).expect_err("chunk exceeds the limit");
    assert_eq!(error, CodecError::InputLimitExceeded { buffered: 4, offered, limit: 4 });
    assert!(error.is_resource_limit());
    assert_eq!(decoder.state(), StreamState::Receiving);
}

#[test]
fn chunked_push_decodes_the_whole_bitstream() {
    let payload = b"acorn-codec chunked stream";
    let compressed = Zlib::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_input_bytes(compressed.len() as u64).with_max_output_bytes(1024);
    let mut decoder = Zlib::decoder(limits);
    let mut output = Vec::new();
    for chunk in compressed.chunks(3) {
        decoder.push(chunk).expect("chunk");
    }

    let facts = decoder.finish(&mut output).expect("finish");
    assert!(facts.complete);
    assert_eq!(output, payload.to_vec());
    assert_eq!(decoder.state(), StreamState::Complete);
}

#[test]
fn push_after_finish_is_rejected() {
    let payload = b"acorn-codec finished stream";
    let compressed = Deflate::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let mut decoder = Deflate::decoder(limits);
    let mut output = Vec::new();
    decoder.push(&compressed).expect("chunk");
    decoder.finish(&mut output).expect("finish");

    let error = decoder.push(b"late").expect_err("late chunk");
    assert_eq!(error, CodecError::InvalidStreamState { state: StreamState::Complete });
}

#[test]
fn finish_twice_is_rejected() {
    let payload = b"acorn-codec single finish";
    let compressed = Deflate::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let mut decoder = Deflate::decoder(limits);
    let mut output = Vec::new();
    decoder.push(&compressed).expect("chunk");
    decoder.finish(&mut output).expect("first finish");

    let error = decoder.finish(&mut output).expect_err("second finish");
    assert_eq!(error, CodecError::InvalidStreamState { state: StreamState::Complete });
}

#[test]
fn a_failed_decode_keeps_the_failure_state() {
    let mut compressed = Zlib::encode(b"acorn-codec broken trailer", DeflateLevel::Store);
    let last = compressed.len() - 1;
    compressed[last] ^= 0xFF;
    let mut decoder = Zlib::decoder(CodecLimits::new());
    let mut output = Vec::new();
    assert!(decoder.finish(&mut output).is_err());

    assert_eq!(decoder.state(), StreamState::Failed);
    let error = decoder.push(&compressed).expect_err("chunk after failure");
    assert_eq!(error, CodecError::InvalidStreamState { state: StreamState::Failed });
    let error = decoder.finish(&mut output).expect_err("finish after failure");
    assert_eq!(error, CodecError::InvalidStreamState { state: StreamState::Failed });
}
