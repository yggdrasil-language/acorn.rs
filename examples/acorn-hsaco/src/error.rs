//! Structural failures and the byte evidence they keep.

use std::fmt::{Display, Formatter};

use acorn_core::{BudgetError, ByteRange};

/// Header table a structural claim belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HsacoHeaderTable {
    /// The program header table addressed by `e_phoff`.
    Program,
    /// The section header table addressed by `e_shoff`.
    Section,
}

impl HsacoHeaderTable {
    /// Table name used in error messages.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Program => "program header table",
            Self::Section => "section header table",
        }
    }
}

/// Byte coordinates for one structural claim.
///
/// `byte_range` is relative to the start of the code object. `section_index`
/// names the section the interval belongs to when the claim is about section
/// content, and is absent for header, table and segment claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HsacoEvidence {
    /// Index of the section the interval belongs to, when it is inside one.
    pub section_index: Option<u32>,
    /// Byte interval inside the code object.
    pub byte_range: ByteRange,
}

impl HsacoEvidence {
    /// Creates evidence for an interval in the object header or a header table.
    pub const fn object(byte_range: ByteRange) -> Self {
        Self { section_index: None, byte_range }
    }

    /// Creates evidence for an interval inside the section at `index`.
    pub const fn section(index: u32, byte_range: ByteRange) -> Self {
        Self { section_index: Some(index), byte_range }
    }

    /// Human-readable location used in error messages.
    fn location(&self) -> String {
        match self.section_index {
            Some(index) => format!("section {index} at byte {}", self.byte_range.start),
            None => format!("byte {}", self.byte_range.start),
        }
    }
}

/// Structural failure found while mapping an ELF64 code object.
///
/// Each variant keeps the coordinates of the bytes that produced it, so a
/// consumer can point at the object instead of re-deriving the location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HsacoLayoutError {
    /// The source does not report a total length.
    UnknownLength,
    /// Fewer than 64 object bytes are available.
    TruncatedHeader {
        /// Bytes the source reports before the header is complete.
        available_bytes: u64,
        /// Interval that should have held the header.
        evidence: HsacoEvidence,
    },
    /// The leading four bytes are not the ELF magic.
    BadMagic {
        /// Magic bytes that were read.
        magic: [u8; 4],
        /// Interval of the magic bytes.
        evidence: HsacoEvidence,
    },
    /// `EI_CLASS` is not the 64-bit class.
    UnsupportedClass {
        /// Class byte that was read.
        class: u8,
        /// Interval of the class byte.
        evidence: HsacoEvidence,
    },
    /// `EI_DATA` is not little endian.
    UnsupportedEndianness {
        /// Data encoding byte that was read.
        data: u8,
        /// Interval of the data encoding byte.
        evidence: HsacoEvidence,
    },
    /// `e_machine` is not the AMDGPU machine.
    MachineMismatch {
        /// Machine word that was read.
        machine: u16,
        /// Interval of the machine word.
        evidence: HsacoEvidence,
    },
    /// A declared entry count needs more bytes than a 64-bit count can address.
    HeaderTableOverflow {
        /// Table the count belongs to.
        table: HsacoHeaderTable,
        /// Entry count declared by the header.
        count: u64,
        /// Declared size of one entry.
        entry_bytes: u64,
        /// Interval of the offset field that addresses the table.
        evidence: HsacoEvidence,
    },
    /// A header table is declared with an entry size below the ELF64 minimum.
    HeaderEntryTooSmall {
        /// Table the entry size belongs to.
        table: HsacoHeaderTable,
        /// Declared entry size.
        entry_bytes: u64,
        /// Smallest entry size the table accepts.
        minimum: u64,
        /// Interval of the entry size field.
        evidence: HsacoEvidence,
    },
    /// A header table runs past the end of the object.
    HeaderTableOutOfBounds {
        /// Table the count belongs to.
        table: HsacoHeaderTable,
        /// Entry count declared by the header.
        count: u64,
        /// Bytes the declared table would occupy.
        table_bytes: u64,
        /// Bytes available from the declared table offset.
        available_bytes: u64,
        /// Offset the table was declared at, for messages.
        offset: u64,
        /// Interval of the offset field that addresses the table.
        evidence: HsacoEvidence,
    },
    /// The last entry of a header table is only partly present.
    TrailingIncompleteEntry {
        /// Table the partial entry belongs to.
        table: HsacoHeaderTable,
        /// Offset where the partial entry starts.
        offset: u64,
        /// Bytes present for the partial entry.
        available_bytes: u64,
        /// Size of one complete entry.
        entry_bytes: u64,
        /// Interval of the offset field that addresses the table.
        evidence: HsacoEvidence,
    },
    /// `e_shstrndx` addresses no section.
    SectionNameTableOutOfBounds {
        /// Section index that was read.
        index: u16,
        /// Sections the object declares.
        section_count: u64,
        /// Interval of the `e_shstrndx` field.
        evidence: HsacoEvidence,
    },
    /// A section's declared payload runs past the end of the object.
    SectionOutOfBounds {
        /// Index of the offending section.
        section_index: u32,
        /// Payload offset declared by the section header.
        offset: u64,
        /// Payload size declared by the section header.
        size: u64,
        /// Bytes the object provides.
        object_bytes: u64,
        /// Interval of the declared payload.
        evidence: HsacoEvidence,
    },
    /// Two sections claim overlapping byte ranges.
    SectionOverlap {
        /// Index of the section that starts first.
        first_index: u32,
        /// Index of the section that starts later.
        second_index: u32,
        /// Interval of the later section.
        evidence: HsacoEvidence,
    },
    /// A program header's declared payload runs past the end of the object.
    SegmentOutOfBounds {
        /// Index of the offending program header.
        segment_index: u32,
        /// File offset declared by the program header.
        offset: u64,
        /// File size declared by the program header.
        file_bytes: u64,
        /// Bytes the object provides.
        object_bytes: u64,
        /// Interval of the declared payload.
        evidence: HsacoEvidence,
    },
    /// The object declares no `.text` section.
    MissingTextSection {
        /// Interval of the section header table offset field.
        evidence: HsacoEvidence,
    },
    /// The `.text` section declares no bytes.
    EmptyTextSection {
        /// Index of the `.text` section.
        section_index: u32,
        /// Interval of the declared section size.
        evidence: HsacoEvidence,
    },
    /// The source returned fewer bytes than requested.
    ShortRead {
        /// Requested byte count.
        expected: u64,
        /// Bytes the source returned.
        actual: u64,
        /// Interval that was requested.
        evidence: HsacoEvidence,
    },
    /// The requested interval is not currently available.
    Unavailable {
        /// Why the interval is unavailable.
        reason: String,
        /// Interval that was requested.
        evidence: HsacoEvidence,
    },
    /// The source read failed.
    ReadFailed {
        /// Underlying failure text.
        message: String,
        /// Interval that was requested.
        evidence: HsacoEvidence,
    },
    /// The read budget was exhausted before the object was mapped.
    Budget(BudgetError),
}

impl Display for HsacoLayoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownLength => write!(f, "HSACO source does not report a total length"),
            Self::TruncatedHeader { available_bytes, evidence } => {
                write!(f, "HSACO ELF header is truncated at {}: {available_bytes} of 64 bytes available", evidence.location())
            }
            Self::BadMagic { magic, evidence } => {
                write!(f, "HSACO ELF magic mismatch at {}: {magic:02X?}", evidence.location())
            }
            Self::UnsupportedClass { class, evidence } => {
                write!(f, "HSACO object at {} is ELF class {class}, only ELF64 (2) is accepted", evidence.location())
            }
            Self::UnsupportedEndianness { data, evidence } => {
                write!(
                    f,
                    "HSACO object at {} declares data encoding {data}, only little endian (1) is accepted",
                    evidence.location()
                )
            }
            Self::MachineMismatch { machine, evidence } => {
                write!(f, "HSACO object at {} declares machine {machine}, only AMDGPU (224) is accepted", evidence.location())
            }
            Self::HeaderTableOverflow { table, count, entry_bytes, evidence } => {
                write!(
                    f,
                    "HSACO {} at {} overflows: {count} entries of {entry_bytes} bytes",
                    table.label(),
                    evidence.location()
                )
            }
            Self::HeaderEntryTooSmall { table, entry_bytes, minimum, evidence } => {
                write!(
                    f,
                    "HSACO {} entry size {entry_bytes} at {} is below the {minimum}-byte minimum",
                    table.label(),
                    evidence.location()
                )
            }
            Self::HeaderTableOutOfBounds { table, count, table_bytes, available_bytes, offset, evidence } => {
                write!(
                    f,
                    "HSACO {} of {count} entries ({table_bytes} bytes) at {} does not fit the object: {available_bytes} bytes available from offset {offset}",
                    table.label(),
                    evidence.location()
                )
            }
            Self::TrailingIncompleteEntry { table, offset, available_bytes, entry_bytes, evidence } => {
                write!(
                    f,
                    "HSACO {} at {} ends with a partial entry: {available_bytes} of {entry_bytes} bytes at offset {offset}",
                    table.label(),
                    evidence.location()
                )
            }
            Self::SectionNameTableOutOfBounds { index, section_count, evidence } => {
                write!(
                    f,
                    "HSACO section name table index {index} at {} addresses {section_count} sections",
                    evidence.location()
                )
            }
            Self::SectionOutOfBounds { section_index, offset, size, object_bytes, evidence } => {
                write!(
                    f,
                    "HSACO section {section_index} payload {offset}+{size} at {} exceeds the {object_bytes}-byte object",
                    evidence.location()
                )
            }
            Self::SectionOverlap { first_index, second_index, evidence } => {
                write!(f, "HSACO section {first_index} overlaps section {second_index} at {}", evidence.location())
            }
            Self::SegmentOutOfBounds { segment_index, offset, file_bytes, object_bytes, evidence } => {
                write!(
                    f,
                    "HSACO segment {segment_index} payload {offset}+{file_bytes} at {} exceeds the {object_bytes}-byte object",
                    evidence.location()
                )
            }
            Self::MissingTextSection { evidence } => {
                write!(f, "HSACO object at {} declares no `.text` section", evidence.location())
            }
            Self::EmptyTextSection { section_index, evidence } => {
                write!(f, "HSACO `.text` section {section_index} at {} is empty", evidence.location())
            }
            Self::ShortRead { expected, actual, evidence } => {
                write!(f, "short read at {}: expected {expected} bytes, read {actual}", evidence.location())
            }
            Self::Unavailable { reason, evidence } => {
                write!(f, "range at {} is unavailable: {reason}", evidence.location())
            }
            Self::ReadFailed { message, evidence } => {
                write!(f, "read at {} failed: {message}", evidence.location())
            }
            Self::Budget(error) => write!(f, "HSACO read budget exhausted: {error}"),
        }
    }
}

impl std::error::Error for HsacoLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            _ => None,
        }
    }
}
