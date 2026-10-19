//! Scalar integer transforms.
//!
//! A scalar transform maps between two integer domains and never produces
//! bytes. Byte representations are separate transforms, so `ZigZag` is not
//! folded into `Leb128`.

/// Mapping between a signed integer domain and its unsigned representation.
pub trait ScalarTransform<T> {
    /// Unsigned counterpart of `T`.
    type Output;

    /// Maps a signed value to its unsigned representation.
    fn encode(value: T) -> Self::Output;

    /// Maps an unsigned representation back to the signed value.
    fn decode(value: Self::Output) -> T;
}
