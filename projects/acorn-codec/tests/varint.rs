use acorn_codec::{Canonicality, CodecError, integer::Leb128};

#[test]
fn uleb128_round_trip() {
    for value in [0u32, 1, 127, 128, 16_383, 16_384, 0xFFFF_FFFF] {
        let mut bytes = Vec::new();
        Leb128::<u32>::encode(value, &mut bytes);
        assert_eq!(Leb128::<u32>::encoded_size(value), bytes.len());
        let decoded = Leb128::<u32>::decode(&bytes).expect("decode");
        assert_eq!(decoded.value, value);
        assert_eq!(decoded.input_consumed, bytes.len());
        assert_eq!(decoded.trailing_input, 0);
        assert_eq!(decoded.canonical, Canonicality::Canonical);
    }
}

#[test]
fn sleb128_i32_round_trip() {
    for value in [0i32, -1, 1, -64, 63, -65, 12_345, -12_345, i32::MIN, i32::MAX] {
        let mut bytes = Vec::new();
        Leb128::<i32>::encode(value, &mut bytes);
        let decoded = Leb128::<i32>::decode(&bytes).expect("decode");
        assert_eq!(decoded.value, value);
        assert_eq!(decoded.canonical, Canonicality::Canonical);
    }
}

#[test]
fn sleb128_i64_round_trip() {
    for value in [0i64, -1, 1, i64::MIN, i64::MAX] {
        let mut bytes = Vec::new();
        Leb128::<i64>::encode(value, &mut bytes);
        let decoded = Leb128::<i64>::decode(&bytes).expect("decode");
        assert_eq!(decoded.value, value);
        assert_eq!(decoded.canonical, Canonicality::Canonical);
    }
}

#[test]
fn uleb128_rejects_overlong_encoding() {
    let overlong = [0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
    assert!(matches!(Leb128::<u32>::decode(&overlong), Err(CodecError::IntegerOverflow { .. })));
}

#[test]
fn uleb128_rejects_bits_above_the_declared_width() {
    assert!(matches!(Leb128::<u32>::decode(&[0xFF, 0xFF, 0xFF, 0xFF, 0x10]), Err(CodecError::IntegerOverflow { .. })));
}

#[test]
fn sleb128_rejects_padding_beyond_i32_without_panicking() {
    let padded = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
    assert!(matches!(Leb128::<i32>::decode(&padded), Err(CodecError::IntegerOverflow { .. })));
}

#[test]
fn leb128_flags_redundant_continuation_bytes() {
    let decoded = Leb128::<u32>::decode(&[0x80, 0x00]).expect("decode");
    assert_eq!(decoded.value, 0);
    assert_eq!(decoded.input_consumed, 2);
    assert_eq!(decoded.canonical, Canonicality::NonCanonical);

    let padded_negative = Leb128::<i32>::decode(&[0xFF, 0xFF, 0xFF, 0xFF, 0x7F]).expect("decode");
    assert_eq!(padded_negative.value, -1);
    assert_eq!(padded_negative.canonical, Canonicality::NonCanonical);
}

#[test]
fn sleb128_keeps_the_two_byte_boundary_canonical() {
    // `64` needs two bytes because bit 6 of the first byte is the sign bit.
    let decoded = Leb128::<i32>::decode(&[0xC0, 0x00]).expect("decode");
    assert_eq!(decoded.value, 64);
    assert_eq!(decoded.canonical, Canonicality::Canonical);
}

#[test]
fn uleb128_reports_trailing_input() {
    let decoded = Leb128::<u32>::decode(&[0x01, 0xFF, 0xFF]).expect("decode");
    assert_eq!(decoded.value, 1);
    assert_eq!(decoded.input_consumed, 1);
    assert_eq!(decoded.trailing_input, 2);
}

#[test]
fn uleb128_truncated_input() {
    assert!(matches!(Leb128::<u32>::decode(&[]), Err(CodecError::InputExhausted { .. })));
    assert!(matches!(Leb128::<u32>::decode(&[0x80]), Err(CodecError::InputExhausted { .. })));
}
