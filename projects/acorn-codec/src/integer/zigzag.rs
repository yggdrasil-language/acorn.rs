//! ZigZag signed-integer mapping (distinct from LEB128 encoding).

use core::marker::PhantomData;

use crate::ScalarTransform;

/// ZigZag transform between signed and unsigned integer domains.
pub struct ZigZag<T>(PhantomData<T>);

macro_rules! zigzag_impl {
    ($signed:ty, $unsigned:ty, $sign_bit:expr) => {
        impl ZigZag<$signed> {
            /// Encodes a signed value as its unsigned ZigZag representation.
            #[inline]
            pub const fn encode(value: $signed) -> $unsigned {
                ((value << 1) ^ (value >> $sign_bit)) as $unsigned
            }

            /// Decodes an unsigned ZigZag representation back to its signed value.
            #[inline]
            pub const fn decode(value: $unsigned) -> $signed {
                ((value >> 1) as $signed) ^ (-((value & 1) as $signed))
            }
        }

        impl ScalarTransform<$signed> for ZigZag<$signed> {
            type Output = $unsigned;

            #[inline]
            fn encode(value: $signed) -> $unsigned {
                ZigZag::<$signed>::encode(value)
            }

            #[inline]
            fn decode(value: $unsigned) -> $signed {
                ZigZag::<$signed>::decode(value)
            }
        }
    };
}

zigzag_impl!(i8, u8, 7);
zigzag_impl!(i16, u16, 15);
zigzag_impl!(i32, u32, 31);
zigzag_impl!(i64, u64, 63);
zigzag_impl!(i128, u128, 127);
