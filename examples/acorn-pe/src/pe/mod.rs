//! `PE/COFF` 解析与原生 / 托管写出。

pub mod clr;
mod native;
mod read;
mod writer;

pub use clr::{
    ClrImageKind, ClrMetadataBuilder, ClrMetadataError, PeWriter, PeWriterError, PeWriterOptions, UserStringsHeap,
    method_body::{FatFormat, IlInstruction, LocalVarFlags, LocalVarType, MethodBody, MethodBodyFlags, parse_method_body_il},
    table_sizes::{CodedIndex, coded_index_size, heap_index_size, table_data_offset, table_index_size, table_row_size},
    tables::{
        ElementType, MemberRefParent, MethodSignature, decode_element_type, decode_member_ref_parent, decode_method_signature, read_idx,
        read_u32_table,
    },
};
pub use native::{NativeDllImport, NativePeImage, NativePeWriter};
pub use read::{
    CliHeader, CoffHeader as PeCoffHeader, DataDirectory, DosHeader, MetadataRoot, OptionalHeader, Pe64Header, Pe64ParseError,
    PeImage, PeParseError, SectionHeader, StreamHeader, TableKind, extract_pe_section, parse_pe, parse_pe64, read_blob, read_compressed_uint,
    read_strings_string, read_user_string, rva_to_offset,
};
