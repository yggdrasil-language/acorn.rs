//! ZigZag signed-integer mapping (distinct from LEB128 encoding).

use core::marker::PhantomData;

use crate::ScalarTransform;

/// ZigZag transform between signed and unsigned integer domains.
pub struct ZigZag<T>(PhantomData<T>);

impl ZigZag<i32> {
    /// Encodes a signed `i32` into an unsigned ZigZag `u32`.
    #[inline]
    pub const fn encode(value: i32) -> u32 {
        ((value << 1) ^ (value >> 31)) as u32
    }

    /// Decodes an unsigned ZigZag `u32` into a signed `i32`.
    #[inline]
    pub const fn decode(value: u32) -> i32 {
        ((value >> 1) as i32) ^ (-((value & 1) as i32))
    }
}

impl ZigZag<i64> {
    /// Encodes a signed `i64` into an unsigned ZigZag `u64`.
    #[inline]
    pub const fn encode(value: i64) -> u64 {
        ((value << 1) ^ (value >> 63)) as u64
    }

    /// Decodes an unsigned ZigZag `u64` into a signed `i64`.
    #[inline]
    pub const fn decode(value: u64) -> i64 {
        ((value >> 1) as i64) ^ (-((value & 1) as i64))
    }
}

impl ScalarTransform<i32> for ZigZag<i32> {
    type Output = u32;

    #[inline]
    fn encode(value: i32) -> u32 {
        ZigZag::<i32>::encode(value)
    }

    #[inline]
    fn decode(value: u32) -> i32 {
        ZigZag::<i32>::decode(value)
    }
}

impl ScalarTransform<i64> for ZigZag<i64> {
    type Output = u64;

    #[inline]
    fn encode(value: i64) -> u64 {
        ZigZag::<i64>::encode(value)
    }

    #[inline]
    fn decode(value: u64) -> i64 {
        ZigZag::<i64>::decode(value)
    }
}
