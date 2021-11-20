#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

mod bytes;
mod error;
mod header;
mod object;

/// Byte-slice validation entry and its error surface.
pub use bytes::{HsacoByteError, validate_hsaco_bytes};
/// Structural failure classification and its byte evidence.
pub use error::{HsacoEvidence, HsacoHeaderTable, HsacoLayoutError};
/// ELF64 constants and the recorded code object header.
pub use header::{
    HSACO_ELF_HEADER_BYTES, HSACO_ELF_MAGIC, HSACO_ELFCLASS64, HSACO_ELFDATA2LSB, HSACO_EM_AMDGPU, HSACO_PROGRAM_HEADER_BYTES,
    HSACO_SECTION_HEADER_BYTES, HSACO_SHT_NOBITS, HSACO_SYMBOL_BYTES, HsacoHeader,
};
/// Bounded object view, section and segment records, summary and the validation entry.
pub use object::{HsacoObject, HsacoSection, HsacoSectionKind, HsacoSegment, HsacoSummary, validate_hsaco};
