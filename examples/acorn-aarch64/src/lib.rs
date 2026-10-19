#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

/// `AArch64` 指令编码、AAPCS64 ABI 与 Android JNI 胶水。
pub mod aarch64;
/// 本 crate 的错误类型与 `Result` 别名。
pub mod error;

macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::new(format!($($arg)*))
    };
}

pub(crate) use err;
