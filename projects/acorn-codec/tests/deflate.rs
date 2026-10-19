use acorn_codec::{
    CodecError, CodecLimits, Ratio,
    bit::{BitOrder, BitWriter},
    compression::{
        Deflate,
        deflate::{DeflateEncoder, DeflateLevel},
    },
};

/// Payload of the checked-in dynamic Huffman fixture below.
fn fox_payload() -> Vec<u8> {
    b"the quick brown fox jumps over the lazy dog. ".repeat(60)
}

/// Raw Deflate of [`fox_payload`] with a dynamic Huffman block (RFC 1951 section 3.2.7).
const DYNAMIC_FOX: &[u8] = &[
    0xed, 0xca, 0xdb, 0x15, 0x40, 0x30, 0x14, 0x05, 0xd1, 0x56, 0x4e, 0x05, 0x7a, 0x22, 0x82, 0x78, 0x5d, 0x22, 0xf1, 0xaa,
    0x9e, 0xa5, 0x06, 0x9f, 0xf3, 0x39, 0x6b, 0x76, 0xea, 0xbc, 0xd6, 0x1c, 0xdc, 0xa0, 0x2a, 0xda, 0x31, 0xab, 0xb1, 0x53,
    0x7d, 0x9e, 0x96, 0x4d, 0xb6, 0xfb, 0xa8, 0xf4, 0xee, 0xb1, 0xbc, 0x2f, 0xd5, 0xd6, 0x16, 0x5f, 0x81, 0xc1, 0x60, 0x30,
    0x18, 0x0c, 0x06, 0x83, 0xc1, 0x3f, 0xe1, 0x07,
];

/// Fixed-Huffman raw Deflate of `Hello` (RFC 1951 section 3.2.6).
const FIXED_HELLO: &[u8] = &[0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];

/// Writes a Huffman code, which travels most-significant bit first.
fn write_code(writer: &mut BitWriter, code: u32, length: u8) {
    let reversed = (0..length).fold(0u32, |acc, index| (acc << 1) | ((code >> index) & 1));
    writer.write_bits(reversed, length).expect("write Huffman code");
}

#[test]
fn deflate_stored_block_round_trip() {
    let payload = b"acorn-codec";
    let compressed = Deflate::encode(payload, DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let facts = Deflate::decode(&compressed, limits).expect("decode");
    assert_eq!(facts.value, payload);
    assert!(facts.fully_consumed(compressed.len()));
}

#[test]
fn deflate_round_trips_every_level() {
    let fox = fox_payload();
    let payloads: &[&[u8]] = &[&[], b"a", b"Hello", b"acorn-codec", fox.as_slice()];
    for level in [DeflateLevel::Store, DeflateLevel::Fast, DeflateLevel::Default] {
        let encoder = DeflateEncoder::new(level);
        for payload in payloads {
            let compressed = encoder.encode(payload);
            let limits = CodecLimits::new().with_max_output_bytes(1 << 20);
            let facts = Deflate::decode(&compressed, limits).expect("decode");
            assert_eq!(facts.value.as_slice(), *payload, "{level:?} must round-trip");
            assert!(facts.fully_consumed(compressed.len()), "{level:?} must consume all input");
        }
    }
}

#[test]
fn deflate_fixed_encoder_matches_the_reference_bytes() {
    // Same bytes as the reference fixed-Huffman encoder for `Hello`.
    let compressed = Deflate::encode(b"Hello", DeflateLevel::Fast);
    assert_eq!(compressed, FIXED_HELLO.to_vec());
}

#[test]
fn deflate_encoder_shrinks_repetition() {
    let payload = fox_payload();
    let stored = Deflate::encode(&payload, DeflateLevel::Store);
    let compressed = Deflate::encode(&payload, DeflateLevel::Default);
    assert!(compressed.len() < payload.len() / 4);
    assert!(compressed.len() < stored.len());
}

#[test]
fn deflate_encoder_handles_long_inputs() {
    let payload: Vec<u8> = (0..70_000u32).map(|index| (index % 7) as u8).collect();
    for level in [DeflateLevel::Store, DeflateLevel::Default] {
        let compressed = Deflate::encode(&payload, level);
        let limits = CodecLimits::new().with_max_output_bytes(1 << 20);
        let facts = Deflate::decode(&compressed, limits).expect("decode");
        assert_eq!(facts.value, payload, "{level:?} must round-trip 70 KiB");
    }
}

#[test]
fn deflate_default_level_emits_a_dynamic_block_when_smaller() {
    let payload = fox_payload();
    let fast = Deflate::encode(&payload, DeflateLevel::Fast);
    let default = Deflate::encode(&payload, DeflateLevel::Default);
    assert_eq!((default[0] >> 1) & 3, 2, "Default emits a dynamic block here");
    assert!(default.len() < fast.len());

    let facts = Deflate::decode(&default, CodecLimits::new().with_max_output_bytes(1 << 20)).expect("decode");
    assert_eq!(facts.value, payload);
}

#[test]
fn deflate_encoder_is_reproducible() {
    let payload = fox_payload();
    for level in [DeflateLevel::Store, DeflateLevel::Fast, DeflateLevel::Default] {
        assert_eq!(Deflate::encode(&payload, level), Deflate::encode(&payload, level), "{level:?} must be reproducible");
    }
}

#[test]
fn deflate_decode_enforces_output_limit() {
    let compressed = Deflate::encode(b"too-large", DeflateLevel::Store);
    let limits = CodecLimits::new().with_max_output_bytes(2);
    let err = Deflate::decode(&compressed, limits).expect_err("limit");
    assert!(matches!(err, CodecError::OutputLimitExceeded { .. }));
}

#[test]
fn deflate_fixed_huffman_round_trip() {
    let limits = CodecLimits::new().with_max_output_bytes(1024);
    let facts = Deflate::decode(FIXED_HELLO, limits).expect("decode fixed Huffman");
    assert_eq!(facts.value, b"Hello");
}

#[test]
fn deflate_dynamic_huffman_round_trip() {
    let limits = CodecLimits::new().with_max_output_bytes(64 * 1024);
    let facts = Deflate::decode(DYNAMIC_FOX, limits).expect("decode dynamic Huffman");
    assert_eq!(facts.value, fox_payload());
    assert!(facts.fully_consumed(DYNAMIC_FOX.len()));
}

#[test]
fn deflate_reports_trailing_input() {
    let mut compressed = Deflate::encode(b"acorn-codec", DeflateLevel::Store);
    compressed.extend_from_slice(b"TRAIL");
    let facts = Deflate::decode(&compressed, CodecLimits::new()).expect("decode");
    assert_eq!(facts.value, b"acorn-codec");
    assert_eq!(facts.trailing_input, 5);
    assert!(!facts.fully_consumed(compressed.len()));
}

#[test]
fn deflate_rejects_truncated_input() {
    let truncated = &FIXED_HELLO[..FIXED_HELLO.len() - 1];
    let error = Deflate::decode(truncated, CodecLimits::new()).expect_err("truncated");
    assert!(matches!(error, CodecError::InputExhausted { .. }));
}

#[test]
fn deflate_enforces_expansion_ratio() {
    let limits = CodecLimits::new().with_max_expansion_ratio(Ratio::new(2, 1));
    let error = Deflate::decode(DYNAMIC_FOX, limits).expect_err("ratio");
    assert!(matches!(error, CodecError::ExpansionRatioExceeded { .. }));
}

#[test]
fn deflate_enforces_block_and_step_limits() {
    let block_limit = CodecLimits::new().with_max_block_count(0);
    let error = Deflate::decode(FIXED_HELLO, block_limit).expect_err("block limit");
    assert!(matches!(error, CodecError::OutputLimitExceeded { .. }));

    let step_limit = CodecLimits::new().with_max_steps(1);
    let error = Deflate::decode(DYNAMIC_FOX, step_limit).expect_err("step limit");
    assert!(matches!(error, CodecError::OutputLimitExceeded { .. }));
}

#[test]
fn deflate_rejects_oversubscribed_code_length_table() {
    let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
    writer.write_bits(1, 1).expect("BFINAL");
    writer.write_bits(2, 2).expect("dynamic block");
    writer.write_bits(0, 5).expect("HLIT");
    writer.write_bits(0, 5).expect("HDIST");
    writer.write_bits(0, 4).expect("HCLEN");
    for length in [1u32, 1, 1, 0] {
        writer.write_bits(length, 3).expect("code length");
    }
    let bytes = writer.finish();

    let error = Deflate::decode(&bytes, CodecLimits::new()).expect_err("oversubscribed");
    assert!(matches!(error, CodecError::InvalidHuffmanTree { .. }));
}

#[test]
fn deflate_rejects_first_code_length_repeat() {
    let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
    writer.write_bits(1, 1).expect("BFINAL");
    writer.write_bits(2, 2).expect("dynamic block");
    writer.write_bits(0, 5).expect("HLIT");
    writer.write_bits(0, 5).expect("HDIST");
    writer.write_bits(0, 4).expect("HCLEN");
    for length in [1u32, 0, 0, 0] {
        writer.write_bits(length, 3).expect("code length");
    }
    // Symbol `16` repeats the previous code length and has none at index 0.
    writer.write_bit(0);
    writer.write_bits(0, 2).expect("repeat count");
    let bytes = writer.finish();

    let error = Deflate::decode(&bytes, CodecLimits::new()).expect_err("repeat");
    assert!(matches!(error, CodecError::InvalidHuffmanTree { .. }));
}

#[test]
fn deflate_rejects_back_reference_into_empty_window() {
    let mut writer = BitWriter::new(BitOrder::LeastSignificantBitFirst);
    writer.write_bits(1, 1).expect("BFINAL");
    writer.write_bits(1, 2).expect("fixed block");
    // Length symbol `257` (length 3) followed by distance symbol `0` (distance 1).
    write_code(&mut writer, 1, 7);
    write_code(&mut writer, 0, 5);
    let bytes = writer.finish();

    let error = Deflate::decode(&bytes, CodecLimits::new()).expect_err("empty window");
    assert!(matches!(error, CodecError::InvalidBackReference { distance: 1, available: 0, .. }));
}
