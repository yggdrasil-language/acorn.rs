//! ELF64 header constants and the recorded code object header.

/// Magic bytes that start every ELF object.
pub const HSACO_ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
/// `EI_CLASS` value of a 64-bit object.
pub const HSACO_ELFCLASS64: u8 = 2;
/// `EI_DATA` value of a little-endian object.
pub const HSACO_ELFDATA2LSB: u8 = 1;
/// `e_machine` value of an AMD GPU code object.
pub const HSACO_EM_AMDGPU: u16 = 224;
/// Size of the ELF64 header in bytes.
pub const HSACO_ELF_HEADER_BYTES: u64 = 64;
/// Size of one ELF64 program header in bytes.
pub const HSACO_PROGRAM_HEADER_BYTES: u64 = 56;
/// Size of one ELF64 section header in bytes.
pub const HSACO_SECTION_HEADER_BYTES: u64 = 64;
/// Size of one ELF64 symbol table entry in bytes.
pub const HSACO_SYMBOL_BYTES: u64 = 24;
/// Section type of a section that occupies no bytes in the file.
pub const HSACO_SHT_NOBITS: u32 = 8;

/// The ELF64 header of a code object, as recorded.
///
/// Every field is decoded from the fixed 64-byte header. `machine` is checked
/// against [`HSACO_EM_AMDGPU`], the version fields are recorded without
/// interpretation, and `entry` is the declared entry virtual address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HsacoHeader {
    /// `EI_VERSION` byte of the identification array.
    pub ident_version: u8,
    /// `e_type` object file type.
    pub object_type: u16,
    /// `e_machine` target architecture, [`HSACO_EM_AMDGPU`] for a valid object.
    pub machine: u16,
    /// `e_version` object version word.
    pub version: u32,
    /// `e_entry` entry virtual address.
    pub entry: u64,
    /// `e_phoff` byte offset of the program header table.
    pub program_header_offset: u64,
    /// `e_phentsize` size of one program header table entry.
    pub program_header_entry_size: u16,
    /// `e_phnum` number of program header table entries.
    pub program_header_count: u16,
    /// `e_shoff` byte offset of the section header table.
    pub section_header_offset: u64,
    /// `e_shentsize` size of one section header table entry.
    pub section_header_entry_size: u16,
    /// `e_shnum` number of section header table entries.
    pub section_header_count: u16,
    /// `e_shstrndx` index of the section that holds the section name table.
    pub section_name_table_index: u16,
    /// `e_flags` target flags.
    pub flags: u32,
}
