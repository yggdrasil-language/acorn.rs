#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

/// 本 crate 的错误类型与 `Result` 别名。
pub mod error;
/// Apple `Mach-O` 64 位可执行镜像构建。
pub mod mach_o;

macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::new(format!($($arg)*))
    };
}

pub(crate) use err;
