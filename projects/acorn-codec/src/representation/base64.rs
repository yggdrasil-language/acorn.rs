//! Standard Base64 encode and decode (RFC 4648).

use crate::{
    SliceCodec,
    error::CodecError,
    facts::{Canonicality, DecodeFacts},
};

const STANDARD_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const URL_SAFE_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Padding policy for Base64 decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    /// Padding characters are required.
    Required,
    /// Padding characters are optional.
    Optional,
    /// Padding characters are forbidden.
    Forbidden,
}

/// Whitespace handling for Base64 decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Reject any ASCII whitespace.
    Reject,
    /// Ignore ASCII whitespace.
    IgnoreAscii,
}

/// Base64 codec profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base64 {
    alphabet: &'static [u8; 64],
    padding: Padding,
    whitespace: Whitespace,
}

impl Base64 {
    /// Standard Base64 (RFC 4648).
    pub const fn standard() -> Self {
        Self { alphabet: STANDARD_ALPHABET, padding: Padding::Required, whitespace: Whitespace::IgnoreAscii }
    }

    /// URL-safe Base64 without padding.
    pub const fn url_safe_no_padding() -> Self {
        Self { alphabet: URL_SAFE_ALPHABET, padding: Padding::Forbidden, whitespace: Whitespace::Reject }
    }

    /// Sets the padding policy.
    pub const fn with_padding(self, padding: Padding) -> Self {
        Self { padding, ..self }
    }

    /// Sets the whitespace policy.
    pub const fn with_whitespace(self, whitespace: Whitespace) -> Self {
        Self { whitespace, ..self }
    }

    /// Encodes `input` as Base64.
    pub fn encode(&self, input: &[u8]) -> Vec<u8> {
        let mut output = Vec::with_capacity((input.len() + 2) / 3 * 4);
        let mut index = 0;
        while index + 3 <= input.len() {
            let chunk = u32::from(input[index]) << 16 | u32::from(input[index + 1]) << 8 | u32::from(input[index + 2]);
            push_quartet(chunk, self.alphabet, &mut output);
            index += 3;
        }
        let remaining = input.len() - index;
        if remaining == 1 {
            let chunk = u32::from(input[index]) << 16;
            output.push(self.alphabet[((chunk >> 18) & 0x3F) as usize]);
            output.push(self.alphabet[((chunk >> 12) & 0x3F) as usize]);
            if self.padding != Padding::Forbidden {
                output.push(b'=');
                output.push(b'=');
            }
        }
        else if remaining == 2 {
            let chunk = u32::from(input[index]) << 16 | u32::from(input[index + 1]) << 8;
            output.push(self.alphabet[((chunk >> 18) & 0x3F) as usize]);
            output.push(self.alphabet[((chunk >> 12) & 0x3F) as usize]);
            output.push(self.alphabet[((chunk >> 6) & 0x3F) as usize]);
            if self.padding != Padding::Forbidden {
                output.push(b'=');
            }
        }
        output
    }

    /// Decodes Base64 `input` into owned bytes.
    pub fn decode(&self, input: &[u8]) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        let trimmed = match self.whitespace {
            Whitespace::IgnoreAscii => trim_ascii_whitespace(input),
            Whitespace::Reject => {
                if let Some(index) = input.iter().position(|byte| byte.is_ascii_whitespace()) {
                    return Err(CodecError::InvalidByte { offset: index, byte: input[index] });
                }
                input
            }
        };

        if trimmed.is_empty() {
            return Ok(DecodeFacts {
                value: Vec::new(),
                input_consumed: input.len(),
                output_produced: 0,
                complete: true,
                trailing_input: 0,
                canonical: Canonicality::Canonical,
            });
        }

        let padding = trimmed.iter().rev().take_while(|&byte| *byte == b'=').count();
        if padding > 2 {
            return Err(CodecError::InvalidPadding { offset: trimmed.len() - padding });
        }
        if trimmed.len() == padding || trimmed.len() % 4 == 1 {
            return Err(CodecError::InvalidPadding { offset: trimmed.len() - 1 });
        }
        let incomplete = trimmed.len() % 4 != 0;
        match self.padding {
            Padding::Required if incomplete => return Err(CodecError::InvalidPadding { offset: trimmed.len() }),
            Padding::Forbidden if padding > 0 => return Err(CodecError::InvalidPadding { offset: trimmed.len() - padding }),
            _ if incomplete && padding > 0 => return Err(CodecError::InvalidPadding { offset: trimmed.len() - padding }),
            _ => {}
        }

        let mut output = Vec::with_capacity(trimmed.len() / 4 * 3);
        let mut buffer = 0u32;
        let mut bits = 0u32;
        for (index, &byte) in trimmed.iter().enumerate() {
            if byte == b'=' {
                if index < trimmed.len() - padding {
                    return Err(CodecError::InvalidPadding { offset: index });
                }
                continue;
            }
            let value = decode_byte(byte, self.alphabet, index)?;
            buffer = (buffer << 6) | value;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push(((buffer >> bits) & 0xFF) as u8);
                buffer &= (1 << bits) - 1;
            }
        }
        let output_produced = output.len();
        let canonical = if buffer == 0 { Canonicality::Canonical } else { Canonicality::NonCanonical };
        Ok(DecodeFacts {
            value: output,
            input_consumed: input.len(),
            output_produced,
            complete: true,
            trailing_input: 0,
            canonical,
        })
    }
}

impl SliceCodec for Base64 {
    type Output = Vec<u8>;

    fn encode(&self, input: &[u8]) -> Vec<u8> {
        Base64::encode(self, input)
    }

    fn decode(&self, input: &[u8]) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
        Base64::decode(self, input)
    }
}

fn push_quartet(chunk: u32, alphabet: &[u8; 64], output: &mut Vec<u8>) {
    output.push(alphabet[((chunk >> 18) & 0x3F) as usize]);
    output.push(alphabet[((chunk >> 12) & 0x3F) as usize]);
    output.push(alphabet[((chunk >> 6) & 0x3F) as usize]);
    output.push(alphabet[(chunk & 0x3F) as usize]);
}

fn decode_byte(byte: u8, alphabet: &[u8; 64], offset: usize) -> Result<u32, CodecError> {
    alphabet.iter().position(|&ch| ch == byte).map(|pos| pos as u32).ok_or(CodecError::InvalidByte { offset, byte })
}

fn trim_ascii_whitespace(input: &[u8]) -> &[u8] {
    let start = input.iter().position(|byte| !byte.is_ascii_whitespace()).unwrap_or(input.len());
    let end = input.iter().rposition(|byte| !byte.is_ascii_whitespace()).map(|index| index + 1).unwrap_or(start);
    input.get(start..end).unwrap_or(&[])
}
