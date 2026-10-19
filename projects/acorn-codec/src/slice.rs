//! One-shot slice transforms.
//!
//! Slice transforms consume a complete byte slice and return either an encoded
//! representation or decode facts. Streaming state belongs to the compression
//! family instead.

use crate::{error::CodecError, facts::DecodeFacts};

/// Byte-slice transform configured by an explicit profile.
pub trait SliceCodec {
    /// Encoded representation produced by [`SliceCodec::encode`].
    type Output;

    /// Encodes `input` using this profile.
    fn encode(&self, input: &[u8]) -> Self::Output;

    /// Decodes `input` using this profile.
    fn decode(&self, input: &[u8]) -> Result<DecodeFacts<Vec<u8>>, CodecError>;
}
