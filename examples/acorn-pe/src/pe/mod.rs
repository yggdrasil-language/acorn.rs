//! `PE/COFF` 解析与原生 `PE32+` 写出。

mod native;
mod read;
mod writer;

pub use native::{NativeDllImport, NativePeImage, NativePeWriter};
pub use read::{
    CliHeader, CoffHeader as PeCoffHeader, DataDirectory, DosHeader, MetadataRoot, OptionalHeader, Pe64Header, Pe64ParseError,
    PeImage, PeParseError, SectionHeader, StreamHeader, TableKind, extract_pe_section, parse_pe, parse_pe64, read_blob, read_compressed_uint,
    read_strings_string, read_user_string, rva_to_offset,
};
