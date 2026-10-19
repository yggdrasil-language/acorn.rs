#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

/// 本 crate 的错误类型与 `Result` 别名。
pub mod error;
/// 各平台最小宿主可执行封装。
pub mod host_exe;

macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::new(format!($($arg)*))
    };
}

pub(crate) use err;
