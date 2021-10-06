//! `WASM` 字节流读取器。
//!
//! 封装游标式字节读取，提供 `LEB128` 变长整数、定长整数、
//! 浮点数与块类型等基础解码能力，供指令解码层复用。

use acorn_codec::{CodecError, integer::Leb128};

use super::WasmBinaryError;

fn map_varint_error(error: CodecError) -> WasmBinaryError {
    match error {
        CodecError::InputExhausted { .. } => WasmBinaryError::UnexpectedEof,
        _ => WasmBinaryError::InvalidLeb128,
    }
}

/// `WASM` 二进制字节读取器，封装游标与各类变长整数解码。
pub struct WasmByteReader<'a> {
    /// 待解码的字节切片。
    bytes: &'a [u8],
    /// 当前读取游标。
    offset: usize,
}

impl<'a> WasmByteReader<'a> {
    /// 创建一个新的读取器，游标位于起始位置。
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// 判断是否已读取到字节流末尾。
    pub fn is_eof(&self) -> bool {
        self.offset >= self.bytes.len()
    }

    /// 返回当前游标位置（已读取的字节数）。
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// 读取单个字节，游标前进一字节。
    pub fn read_u8(&mut self) -> Result<u8, WasmBinaryError> {
        let value = *self.bytes.get(self.offset).ok_or(WasmBinaryError::UnexpectedEof)?;
        self.offset += 1;
        Ok(value)
    }

    /// 读取指定长度的字节切片，游标前进对应字节数。
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], WasmBinaryError> {
        let end = self.offset.checked_add(len).ok_or(WasmBinaryError::UnexpectedEof)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(WasmBinaryError::UnexpectedEof)?;
        self.offset = end;
        Ok(bytes)
    }

    /// 读取无符号 `LEB128` 编码的 `u32`。
    pub fn read_uleb128(&mut self) -> Result<u32, WasmBinaryError> {
        let decoded = Leb128::<u32>::decode(self.bytes.get(self.offset..).unwrap_or(&[])).map_err(map_varint_error)?;
        self.offset += decoded.input_consumed;
        Ok(decoded.value)
    }

    /// 读取有符号 `LEB128` 编码的 `i32`。
    pub fn read_sleb128_i32(&mut self) -> Result<i32, WasmBinaryError> {
        let decoded = Leb128::<i32>::decode(self.bytes.get(self.offset..).unwrap_or(&[])).map_err(map_varint_error)?;
        self.offset += decoded.input_consumed;
        Ok(decoded.value)
    }

    /// 读取有符号 `LEB128` 编码的 `i64`。
    pub fn read_sleb128_i64(&mut self) -> Result<i64, WasmBinaryError> {
        let decoded = Leb128::<i64>::decode(self.bytes.get(self.offset..).unwrap_or(&[])).map_err(map_varint_error)?;
        self.offset += decoded.input_consumed;
        Ok(decoded.value)
    }

    /// 读取小端序 `f32`。
    pub fn read_f32(&mut self) -> Result<f32, WasmBinaryError> {
        let bytes = self.read_bytes(4)?;
        let mut array = [0u8; 4];
        array.copy_from_slice(bytes);
        Ok(f32::from_le_bytes(array))
    }

    /// 读取小端序 `f64`。
    pub fn read_f64(&mut self) -> Result<f64, WasmBinaryError> {
        let bytes = self.read_bytes(8)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(f64::from_le_bytes(array))
    }

    /// 读取长度前缀 `UTF-8` 字符串。
    ///
    /// 先读无符号 `LEB128` 作为字节长度，再读取对应字节并解析为 `String`。
    /// 字节序列不是合法 `UTF-8` 时返回 `WasmBinaryError::InvalidUtf8Name`。
    pub fn read_string(&mut self) -> Result<String, WasmBinaryError> {
        let len = self.read_uleb128()? as usize;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| WasmBinaryError::InvalidUtf8Name)
    }

    /// 读取块类型，返回其有符号整数表示。
    ///
    /// 单字节块类型（高位为 0）按 `7` 位有符号 `LEB128` 解码并符号扩展：
    /// 例如 `0x40` 解码为 `-64`（空块类型），`0x7F` 解码为 `-1`（`i32`）。
    /// 多字节块类型按简化的有符号 `LEB128` 继续读取。
    pub fn read_block_type(&mut self) -> Result<i64, WasmBinaryError> {
        let byte = self.read_u8()?;
        if byte & 0x80 == 0 {
            let value = (byte & 0x7F) as i64;
            if byte & 0x40 != 0 { Ok(value | (!0i64 << 7)) } else { Ok(value) }
        }
        else {
            let mut result = (byte & 0x7F) as i64;
            let mut shift = 7u32;
            loop {
                let b = self.read_u8()?;
                result |= ((b & 0x7F) as i64) << shift;
                if b & 0x80 == 0 {
                    break;
                }
                shift += 7;
            }
            Ok(result)
        }
    }
}
