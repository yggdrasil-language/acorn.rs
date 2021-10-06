//! 本 crate 的错误类型与 `Result` 别名。

use std::fmt;

/// `DEX` 编码或读取错误，携带诊断文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    /// 由诊断文本创建错误。
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// 本 crate 的 `Result` 别名。
pub type Result<T> = std::result::Result<T, Error>;
