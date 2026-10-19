use acorn_codec::{ScalarTransform, integer::ZigZag};

#[test]
fn zigzag_i32_round_trip() {
    for value in [0i32, -1, 1, i32::MIN, i32::MAX] {
        let encoded = ZigZag::<i32>::encode(value);
        assert_eq!(ZigZag::<i32>::decode(encoded), value);
    }
}

#[test]
fn zigzag_i64_round_trip() {
    for value in [0i64, -1, 1, i64::MIN, i64::MAX] {
        let encoded = ZigZag::<i64>::encode(value);
        assert_eq!(ZigZag::<i64>::decode(encoded), value);
    }
}

#[test]
fn zigzag_covers_every_integer_width() {
    for value in [0i8, -1, 1, i8::MIN, i8::MAX] {
        assert_eq!(ZigZag::<i8>::decode(ZigZag::<i8>::encode(value)), value);
    }
    for value in [0i16, -1, 1, i16::MIN, i16::MAX] {
        assert_eq!(ZigZag::<i16>::decode(ZigZag::<i16>::encode(value)), value);
    }
    for value in [0i128, -1, 1, i128::MIN, i128::MAX] {
        assert_eq!(ZigZag::<i128>::decode(ZigZag::<i128>::encode(value)), value);
    }
}

#[test]
fn zigzag_matches_the_reference_mapping() {
    // `0` maps to `0`, `-1` maps to `1`, and the type minimum maps to the unsigned maximum.
    assert_eq!(ZigZag::<i8>::encode(0), 0);
    assert_eq!(ZigZag::<i8>::encode(-1), 1);
    assert_eq!(ZigZag::<i8>::encode(1), 2);
    assert_eq!(ZigZag::<i8>::encode(i8::MIN), u8::MAX);
    assert_eq!(ZigZag::<i128>::encode(i128::MIN), u128::MAX);
    assert_eq!(ZigZag::<i128>::decode(u128::MAX), i128::MIN);
}

#[test]
fn zigzag_exposes_the_scalar_family() {
    assert_eq!(<ZigZag<i32> as ScalarTransform<i32>>::encode(-1), 1);
    assert_eq!(<ZigZag<i32> as ScalarTransform<i32>>::decode(1), -1);
    assert_eq!(<ZigZag<i64> as ScalarTransform<i64>>::encode(i64::MIN), u64::MAX);
    assert_eq!(<ZigZag<i8> as ScalarTransform<i8>>::encode(i8::MIN), u8::MAX);
    assert_eq!(<ZigZag<i128> as ScalarTransform<i128>>::decode(u128::MAX), i128::MIN);
}
