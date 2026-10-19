use acorn_codec::{CodecError, representation::Hex};

#[test]
fn hex_round_trip() {
    let codec = Hex::lowercase();
    let inputs: &[&[u8]] = &[&[], b"a", b"acorn", b"\x00\xff\x10"];
    for input in inputs {
        let encoded = codec.encode(input);
        let decoded = codec.decode(&encoded).expect("decode");
        assert_eq!(decoded.value, *input);
    }
}

#[test]
fn hex_decode_ignores_whitespace_and_mixed_case() {
    let codec = Hex::any_case();
    assert_eq!(codec.decode(b"4D 61 6E").expect("decode").value, b"Man");
    assert!(matches!(codec.decode(b"4d6g"), Err(CodecError::InvalidByte { .. })));
}

#[test]
fn hex_rejects_odd_digit_count() {
    let codec = Hex::lowercase();
    let error = codec.decode(b"abc").expect_err("odd digit count");
    assert!(matches!(error, CodecError::InvalidByte { offset: 2, byte: b'c' }));
}

#[test]
fn hex_rejects_whitespace_when_strict() {
    let error = Hex::strict().decode(b"4d 61").expect_err("whitespace");
    assert!(matches!(error, CodecError::InvalidByte { offset: 2, byte: b' ' }));
}
