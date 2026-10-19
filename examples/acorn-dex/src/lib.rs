#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

/// Android `DEX`（`dex/035`）读写与合并。
pub mod dex;
/// 本 crate 的错误类型与 `Result` 别名。
pub mod error;

macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::new(format!($($arg)*))
    };
}

pub(crate) use err;
