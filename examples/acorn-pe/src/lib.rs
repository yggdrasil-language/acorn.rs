#![doc = include_str!("readme.md")]

/// `COFF` 对象文件模型与写出器。
pub mod coff;
/// `ELF64` 可执行与共享库镜像。
pub mod elf;
/// `MSIL` 文本模型与方法体编码（`CLR` `PE` 写入依赖）。
pub mod msil;
/// `PE/COFF` 解析、原生 `PE32+` 与托管 `CLR` 写出。
pub mod pe;
