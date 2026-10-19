#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

/// 本 crate 的错误类型与 `Result` 别名。
pub mod error;
/// x86-64 原生 `ELF64` 可执行镜像构建。
pub mod native_elf;
/// x86-64 原生 `PE32+` 可执行镜像构建。
pub mod native_pe;
/// x86-64 指令编码、ABI 与 `COFF` 布局。
pub mod x86_64;

macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::new(format!($($arg)*))
    };
}

pub(crate) use err;
