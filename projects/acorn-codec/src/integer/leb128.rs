//! LEB128 variable-length integer encode and decode.

use core::marker::PhantomData;

use crate::{
    error::CodecError,
    facts::{Canonicality, DecodeFacts},
};

/// LEB128 profile for format-specific width and signedness rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leb128Profile {
    /// Maximum bit width.
    pub width: u32,
    /// Whether values are signed.
    pub signed: bool,
}

impl Leb128Profile {
    /// Wasm unsigned 32-bit LEB128.
    pub const WASM_U32: Self = Self { width: 32, signed: false };
    /// Wasm signed 32-bit LEB128.
    pub const WASM_I32: Self = Self { width: 32, signed: true };
    /// Wasm signed 64-bit LEB128.
    pub const WASM_I64: Self = Self { width: 64, signed: true };
}

/// LEB128 codec for a specific integer type.
pub struct Leb128<T>(PhantomData<T>);

impl Leb128<u32> {
    /// Decodes an unsigned LEB128 `u32` from `input`.
    pub fn decode(input: &[u8]) -> Result<DecodeFacts<u32>, CodecError> {
        Self::decode_with_profile(input, Leb128Profile::WASM_U32)
    }

    /// Decodes with an explicit profile.
    pub fn decode_with_profile(input: &[u8], profile: Leb128Profile) -> Result<DecodeFacts<u32>, CodecError> {
        if profile.signed || profile.width != 32 {
            return Err(CodecError::InternalInvariantViolation);
        }
        let decoded = decode_unsigned(input, 32)?;
        Ok(DecodeFacts {
            value: decoded.value as u32,
            input_consumed: decoded.input_consumed,
            output_produced: 0,
            complete: true,
            trailing_input: input.len() - decoded.input_consumed,
            canonical: decoded.canonical,
        })
    }

    /// Encodes `value` as unsigned LEB128 into `output`.
    pub fn encode(value: u32, output: &mut Vec<u8>) {
        let mut remaining = value;
        loop {
            let mut byte = (remaining & 0x7F) as u8;
            remaining >>= 7;
            if remaining != 0 {
                byte |= 0x80;
            }
            output.push(byte);
            if remaining == 0 {
                break;
            }
        }
    }

    /// Returns the number of bytes required to encode `value`.
    pub fn encoded_size(value: u32) -> usize {
        let mut size = 1;
        let mut remaining = value >> 7;
        while remaining != 0 {
            size += 1;
            remaining >>= 7;
        }
        size
    }
}

impl Leb128<i32> {
    /// Decodes a signed LEB128 `i32` from `input`.
    pub fn decode(input: &[u8]) -> Result<DecodeFacts<i32>, CodecError> {
        Self::decode_with_profile(input, Leb128Profile::WASM_I32)
    }

    /// Decodes with an explicit profile.
    pub fn decode_with_profile(input: &[u8], profile: Leb128Profile) -> Result<DecodeFacts<i32>, CodecError> {
        if !profile.signed || profile.width != 32 {
            return Err(CodecError::InternalInvariantViolation);
        }
        let decoded = decode_signed(input, 32)?;
        Ok(DecodeFacts {
            value: decoded.value as i32,
            input_consumed: decoded.input_consumed,
            output_produced: 0,
            complete: true,
            trailing_input: input.len() - decoded.input_consumed,
            canonical: decoded.canonical,
        })
    }

    /// Encodes `value` as signed LEB128 `i32` into `output`.
    pub fn encode(mut value: i32, output: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
            output.push(if done { byte } else { byte | 0x80 });
            if done {
                break;
            }
        }
    }
}

impl Leb128<i64> {
    /// Decodes a signed LEB128 `i64` from `input`.
    pub fn decode(input: &[u8]) -> Result<DecodeFacts<i64>, CodecError> {
        Self::decode_with_profile(input, Leb128Profile::WASM_I64)
    }

    /// Decodes with an explicit profile.
    pub fn decode_with_profile(input: &[u8], profile: Leb128Profile) -> Result<DecodeFacts<i64>, CodecError> {
        if !profile.signed || profile.width != 64 {
            return Err(CodecError::InternalInvariantViolation);
        }
        let decoded = decode_signed(input, 64)?;
        Ok(DecodeFacts {
            value: decoded.value,
            input_consumed: decoded.input_consumed,
            output_produced: 0,
            complete: true,
            trailing_input: input.len() - decoded.input_consumed,
            canonical: decoded.canonical,
        })
    }

    /// Encodes `value` as signed LEB128 `i64` into `output`.
    pub fn encode(mut value: i64, output: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
            output.push(if done { byte } else { byte | 0x80 });
            if done {
                break;
            }
        }
    }
}

struct Decoded<T> {
    value: T,
    input_consumed: usize,
    canonical: Canonicality,
}

fn decode_unsigned(input: &[u8], width: u32) -> Result<Decoded<u64>, CodecError> {
    let mut result = 0u64;
    let mut shift = 0u32;
    for (index, &byte) in input.iter().enumerate() {
        if shift >= width {
            return Err(CodecError::IntegerOverflow { offset: index, width });
        }
        let low = u64::from(byte & 0x7F);
        let remaining = width - shift;
        if remaining < 7 && (low >> remaining) != 0 {
            return Err(CodecError::IntegerOverflow { offset: index, width });
        }
        result |= low << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            let consumed = index + 1;
            let canonical = canonicality(result as i64, &input[..consumed], false);
            return Ok(Decoded { value: result, input_consumed: consumed, canonical });
        }
    }
    Err(CodecError::InputExhausted { offset: input.len(), needed: 1 })
}

fn decode_signed(input: &[u8], width: u32) -> Result<Decoded<i64>, CodecError> {
    let mut result = 0i64;
    let mut shift = 0u32;
    for (index, &byte) in input.iter().enumerate() {
        if shift >= width {
            return Err(CodecError::IntegerOverflow { offset: index, width });
        }
        let low = i64::from(byte & 0x7F);
        let remaining = width - shift;
        if remaining < 7 {
            let sign = (low >> 6) & 1;
            let used = low & ((1i64 << remaining) - 1);
            let padding = low >> remaining;
            let expected = if sign == 1 { (1i64 << (7 - remaining)) - 1 } else { 0 };
            if padding != expected {
                return Err(CodecError::IntegerOverflow { offset: index, width });
            }
            result |= used << shift;
            shift += remaining;
            // The value's sign now sits at the top of the declared width.
            if shift < 64 && (result >> (shift - 1)) & 1 == 1 {
                result |= (-1i64) << shift;
            }
        }
        else {
            result |= low << shift;
            shift += 7;
        }
        if byte & 0x80 == 0 {
            if remaining >= 7 && (byte & 0x40) != 0 {
                // Sign-extend a negative value up to the accumulator width.
                result |= (-1i64) << shift;
            }
            let consumed = index + 1;
            let canonical = canonicality(result, &input[..consumed], true);
            return Ok(Decoded { value: result, input_consumed: consumed, canonical });
        }
    }
    Err(CodecError::InputExhausted { offset: input.len(), needed: 1 })
}

/// Whether the consumed bytes equal the canonical encoding of `value`.
fn canonicality(value: i64, consumed: &[u8], signed: bool) -> Canonicality {
    let mut buffer = [0u8; 10];
    let length = canonical_bytes(value, signed, &mut buffer);
    if length == consumed.len() && buffer[..length] == *consumed { Canonicality::Canonical } else { Canonicality::NonCanonical }
}

fn canonical_bytes(value: i64, signed: bool, buffer: &mut [u8; 10]) -> usize {
    let mut length = 0usize;
    if signed {
        let mut remaining = value;
        loop {
            let byte = (remaining & 0x7F) as u8;
            remaining >>= 7;
            let done = (remaining == 0 && (byte & 0x40) == 0) || (remaining == -1 && (byte & 0x40) != 0);
            buffer[length] = if done { byte } else { byte | 0x80 };
            length += 1;
            if done {
                return length;
            }
        }
    }
    let mut remaining = value as u64;
    loop {
        let byte = (remaining & 0x7F) as u8;
        remaining >>= 7;
        buffer[length] = if remaining != 0 { byte | 0x80 } else { byte };
        length += 1;
        if remaining == 0 {
            return length;
        }
    }
}
