//! Hexadecimal encode and decode.

use crate::{
    SliceCodec,
    error::CodecError,
    facts::{Canonicality, DecodeFacts},
};

/// Whitespace handling for hex decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Reject any ASCII whitespace.
    Reject,
    /// Ignore ASCII whitespace.
    IgnoreAscii,
}

/// Hex codec profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hex {
    lowercase: bool,
    whitespace: Whitespace,
}

impl Hex {
    /// Lowercase hex encoding, whitespace ignored on decode.
    pub const fn lowercase() -> Self {
        Self { lowercase: true, whitespace: Whitespace::IgnoreAscii }
    }

    /// Strict hex: reject whitespace, accept any case.
    pub const fn strict() -> Self {
        Self { lowercase: false, whitespace: Whitespace::Reject }
    }

    /// Accept any case on decode.
    pub const fn any_case() -> Self {
        Self { lowercase: false, whitespace: Whitespace::IgnoreAscii }
    }

    /// Sets the whitespace policy.
    pub const fn with_whitespace(self, whitespace: Whitespace) -> Self {
        Self { whitespace, ..self }
    }

    /// Encodes `input` as hexadecimal.
    pub fn encode(&self, input: &[u8]) -> Vec<u8> {
        const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";
        const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
        let table = if self.lowercase { HEX_LOWER } else { HEX_UPPER };
        let mut output = Vec::with_capacity(input.len() * 2);
        for &byte in input {
            output.push(table[(byte >> 4) as usize]);
            output.push(table[(byte & 0x0F) as usize]);
        }
        output
    }

    /// Decodes hexadecimal `input` into owned bytes.
    pub fn decode(&self, input: &[u8]) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        let mut digits = Vec::new();
        let mut last = None;
        for (index, &byte) in input.iter().enumerate() {
            if byte.is_ascii_whitespace() {
                if self.whitespace == Whitespace::Reject {
                    return Err(CodecError::InvalidByte { offset: index, byte });
                }
                continue;
            }
            digits.push(decode_digit(byte, index)?);
            last = Some((index, byte));
        }
        if digits.len() % 2 != 0 {
            let (offset, byte) = last.unwrap_or((0, 0));
            return Err(CodecError::InvalidByte { offset, byte });
        }
        let mut output = Vec::with_capacity(digits.len() / 2);
        for pair in digits.chunks_exact(2) {
            output.push((pair[0] << 4) | pair[1]);
        }
        let output_produced = output.len();
        Ok(DecodeFacts {
            value: output,
            input_consumed: input.len(),
            output_produced,
            complete: true,
            trailing_input: 0,
            canonical: Canonicality::Canonical,
        })
    }
}

impl SliceCodec for Hex {
    type Output = Vec<u8>;

    fn encode(&self, input: &[u8]) -> Vec<u8> {
        Hex::encode(self, input)
    }

    fn decode(&self, input: &[u8]) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        Hex::decode(self, input)
    }
}

fn decode_digit(byte: u8, offset: usize) -> Result<u8, CodecError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(CodecError::InvalidByte { offset, byte }),
    }
}
