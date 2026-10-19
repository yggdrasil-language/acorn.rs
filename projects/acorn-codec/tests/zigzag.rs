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

/// The scalar family is usable through the trait without naming `ZigZag`.
fn round_trip_through_family<T>()
where
    ZigZag<T>: ScalarTransform<T>,
{
}

#[test]
fn zigzag_exposes_the_scalar_family() {
    assert_eq!(<ZigZag<i32> as ScalarTransform<i32>>::encode(-1), 1);
    assert_eq!(<ZigZag<i32> as ScalarTransform<i32>>::decode(1), -1);
    assert_eq!(<ZigZag<i64> as ScalarTransform<i64>>::encode(i64::MIN), u64::MAX);
    round_trip_through_family::<i32>();
}
