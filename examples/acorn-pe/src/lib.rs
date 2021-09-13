#![doc = include_str!("readme.md")]

/// `COFF` 对象文件模型与写出器。
pub mod coff;
/// `ELF64` 可执行与共享库镜像。
pub mod elf;
/// `PE/COFF` 解析与原生 `PE32+` 写出。
pub mod pe;
