//! Bounded code object view and the structural validation entry.

use acorn_core::{BudgetTracker, ByteRange};
use acorn_source::{ByteSource, ReadError, SourceRange};

use crate::{
    error::{HsacoEvidence, HsacoHeaderTable, HsacoLayoutError},
    header::{
        HSACO_ELF_HEADER_BYTES, HSACO_ELF_MAGIC, HSACO_ELFCLASS64, HSACO_ELFDATA2LSB, HSACO_EM_AMDGPU,
        HSACO_PROGRAM_HEADER_BYTES, HSACO_SECTION_HEADER_BYTES, HSACO_SHT_NOBITS, HSACO_SYMBOL_BYTES, HsacoHeader,
    },
};

/// Class of a section, derived from the name its header points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HsacoSectionKind {
    /// `.text`: the machine code the code object carries.
    Text,
    /// `.rodata`: read-only data.
    Rodata,
    /// `.symtab`: the symbol table.
    SymbolTable,
    /// `.strtab` or `.shstrtab`: a string table.
    StringTable,
    /// `.note`: note records, kept as a name and an interval.
    Note,
    /// `.amdgpu_metadata`: the metadata blob, kept as an interval only.
    AmdgpuMetadata,
    /// Any other section, kept verbatim.
    Other,
}

impl HsacoSectionKind {
    /// Classifies a section by its resolved name.
    pub fn from_name(name: &str) -> Self {
        match name {
            ".text" => Self::Text,
            ".rodata" => Self::Rodata,
            ".symtab" => Self::SymbolTable,
            ".strtab" | ".shstrtab" => Self::StringTable,
            ".note" => Self::Note,
            ".amdgpu_metadata" => Self::AmdgpuMetadata,
            _ => Self::Other,
        }
    }
}

/// One section: its name, its class and the bytes it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HsacoSection {
    /// Position of the section in the section header table.
    pub index: u32,
    /// Name the section header points at, empty when it cannot be resolved.
    pub name: String,
    /// Classification derived from `name`.
    pub kind: HsacoSectionKind,
    /// File interval of the section payload.
    ///
    /// A section that occupies no file bytes, `SHT_NOBITS` included, records an
    /// empty interval at its declared offset.
    pub byte_range: ByteRange,
}

/// One program header: its type, flags and the file interval it maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HsacoSegment {
    /// Position of the program header in its table.
    pub index: u32,
    /// `p_type` segment type, recorded without interpretation.
    pub segment_type: u32,
    /// `p_flags` segment flags, recorded without interpretation.
    pub flags: u32,
    /// File interval the segment maps.
    pub byte_range: ByteRange,
}

/// What a structurally valid code object declares about its own layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HsacoSummary {
    /// Header fields that were read.
    pub header: HsacoHeader,
    /// Program headers in table order.
    pub segments: Vec<HsacoSegment>,
    /// Sections in section header table order.
    pub sections: Vec<HsacoSection>,
    /// File interval of the `.text` section.
    pub text: ByteRange,
    /// Sections the object declares.
    pub section_count: u64,
    /// Symbol table entries when a `.symtab` section can be measured.
    pub symbol_count: Option<u64>,
    /// File interval of the `.amdgpu_metadata` section, when it is present.
    pub amdgpu_metadata: Option<ByteRange>,
}

/// Bounded view over one ELF64 code object: header, header tables and sections.
///
/// Opening the object reads only the fixed 64-byte header and checks the
/// geometry of both header tables. Program headers, section headers and the
/// section name table are read on demand through [`HsacoObject::validate`], each
/// settled against the caller's ledger.
#[derive(Debug)]
pub struct HsacoObject<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    header: HsacoHeader,
    length: u64,
}

impl<'a, S: ByteSource + ?Sized> HsacoObject<'a, S> {
    /// Maps the ELF64 header and bounds both header tables.
    ///
    /// Magic, class, endianness, machine and the two table geometries are
    /// checked here. Entries themselves are not read, so an entry-level failure
    /// is reported by [`HsacoObject::validate`] with its own evidence.
    pub fn open(source: &'a S, ledger: &mut BudgetTracker) -> Result<Self, HsacoLayoutError> {
        let length = source.length().ok_or(HsacoLayoutError::UnknownLength)?;
        if length < HSACO_ELF_HEADER_BYTES {
            return Err(HsacoLayoutError::TruncatedHeader {
                available_bytes: length,
                evidence: HsacoEvidence::object(ByteRange { start: 0, len: length }),
            });
        }

        let bytes = read_exact(
            source,
            ledger,
            0,
            HSACO_ELF_HEADER_BYTES,
            HsacoEvidence::object(ByteRange { start: 0, len: HSACO_ELF_HEADER_BYTES }),
        )?;

        let magic: [u8; 4] = bytes[0..4].try_into().expect("4-byte ELF magic");
        if magic != HSACO_ELF_MAGIC {
            return Err(HsacoLayoutError::BadMagic { magic, evidence: HsacoEvidence::object(ByteRange { start: 0, len: 4 }) });
        }

        let class = bytes[4];
        if class != HSACO_ELFCLASS64 {
            return Err(HsacoLayoutError::UnsupportedClass {
                class,
                evidence: HsacoEvidence::object(ByteRange { start: 4, len: 1 }),
            });
        }

        let data = bytes[5];
        if data != HSACO_ELFDATA2LSB {
            return Err(HsacoLayoutError::UnsupportedEndianness {
                data,
                evidence: HsacoEvidence::object(ByteRange { start: 5, len: 1 }),
            });
        }

        let machine = read_u16(&bytes, 18);
        if machine != HSACO_EM_AMDGPU {
            return Err(HsacoLayoutError::MachineMismatch {
                machine,
                evidence: HsacoEvidence::object(ByteRange { start: 18, len: 2 }),
            });
        }

        let header = HsacoHeader {
            ident_version: bytes[6],
            object_type: read_u16(&bytes, 16),
            machine,
            version: read_u32(&bytes, 20),
            entry: read_u64(&bytes, 24),
            program_header_offset: read_u64(&bytes, 32),
            program_header_entry_size: read_u16(&bytes, 54),
            program_header_count: read_u16(&bytes, 56),
            section_header_offset: read_u64(&bytes, 40),
            section_header_entry_size: read_u16(&bytes, 58),
            section_header_count: read_u16(&bytes, 60),
            section_name_table_index: read_u16(&bytes, 62),
            flags: read_u32(&bytes, 48),
        };

        table_bytes(
            length,
            TableLayout {
                table: HsacoHeaderTable::Program,
                offset: header.program_header_offset,
                count: u64::from(header.program_header_count),
                entry_bytes: u64::from(header.program_header_entry_size),
                minimum_entry_bytes: HSACO_PROGRAM_HEADER_BYTES,
                offset_evidence: HsacoEvidence::object(ByteRange { start: 32, len: 8 }),
                entry_size_evidence: HsacoEvidence::object(ByteRange { start: 54, len: 2 }),
            },
        )?;
        table_bytes(
            length,
            TableLayout {
                table: HsacoHeaderTable::Section,
                offset: header.section_header_offset,
                count: u64::from(header.section_header_count),
                entry_bytes: u64::from(header.section_header_entry_size),
                minimum_entry_bytes: HSACO_SECTION_HEADER_BYTES,
                offset_evidence: HsacoEvidence::object(ByteRange { start: 40, len: 8 }),
                entry_size_evidence: HsacoEvidence::object(ByteRange { start: 58, len: 2 }),
            },
        )?;

        Ok(Self { source, header, length })
    }

    /// Header fields read when the object was opened.
    pub const fn header(&self) -> HsacoHeader {
        self.header
    }

    /// Validates every program header, section header and named section.
    ///
    /// Section payloads must fit the object, sections that occupy file bytes may
    /// not overlap, `.text` must exist and declare bytes, and the symbol table
    /// must be measurable. No instruction or metadata semantics are inspected.
    pub fn validate(&self, ledger: &mut BudgetTracker) -> Result<HsacoSummary, HsacoLayoutError> {
        let headers = self.section_headers(ledger)?;
        let segments = self.segments(ledger)?;
        self.check_section_bounds(&headers)?;
        self.check_section_overlap(&headers)?;
        let names = self.section_names(ledger, &headers)?;

        let mut sections = Vec::with_capacity(headers.len());
        let mut text = None;
        let mut symbol_count = None;
        let mut amdgpu_metadata = None;

        for (position, header) in headers.iter().enumerate() {
            let index = position as u32;
            let name = names.get(position).cloned().unwrap_or_default();
            let kind = HsacoSectionKind::from_name(&name);
            let byte_range = header.byte_range();

            match kind {
                HsacoSectionKind::Text if text.is_none() => text = Some((index, byte_range)),
                HsacoSectionKind::SymbolTable if symbol_count.is_none() => {
                    let entry_bytes = if header.entry_size == 0 { HSACO_SYMBOL_BYTES } else { header.entry_size };
                    symbol_count = Some(header.size / entry_bytes);
                }
                HsacoSectionKind::AmdgpuMetadata if amdgpu_metadata.is_none() => amdgpu_metadata = Some(byte_range),
                _ => {}
            }

            sections.push(HsacoSection { index, name, kind, byte_range });
        }

        let (text_index, text) = text
            .ok_or(HsacoLayoutError::MissingTextSection { evidence: HsacoEvidence::object(ByteRange { start: 40, len: 8 }) })?;
        if text.len == 0 {
            return Err(HsacoLayoutError::EmptyTextSection {
                section_index: text_index,
                evidence: self.section_field(text_index, 32),
            });
        }

        Ok(HsacoSummary {
            header: self.header,
            segments,
            section_count: sections.len() as u64,
            sections,
            text,
            symbol_count,
            amdgpu_metadata,
        })
    }

    /// Reads and decodes every entry of the section header table.
    fn section_headers(&self, ledger: &mut BudgetTracker) -> Result<Vec<SectionHeader>, HsacoLayoutError> {
        let count = u64::from(self.header.section_header_count);
        let entry_bytes = u64::from(self.header.section_header_entry_size);
        let mut headers = Vec::with_capacity(count as usize);
        for index in 0..count {
            let start = self.header.section_header_offset + index * entry_bytes;
            let bytes = read_exact(
                self.source,
                ledger,
                start,
                HSACO_SECTION_HEADER_BYTES,
                HsacoEvidence::object(ByteRange { start, len: HSACO_SECTION_HEADER_BYTES }),
            )?;
            headers.push(SectionHeader {
                name_offset: read_u32(&bytes, 0),
                section_type: read_u32(&bytes, 4),
                offset: read_u64(&bytes, 24),
                size: read_u64(&bytes, 32),
                entry_size: read_u64(&bytes, 56),
            });
        }
        Ok(headers)
    }

    /// Reads and decodes every entry of the program header table.
    fn segments(&self, ledger: &mut BudgetTracker) -> Result<Vec<HsacoSegment>, HsacoLayoutError> {
        let count = u64::from(self.header.program_header_count);
        let entry_bytes = u64::from(self.header.program_header_entry_size);
        let mut segments = Vec::with_capacity(count as usize);
        for index in 0..count {
            let start = self.header.program_header_offset + index * entry_bytes;
            let bytes = read_exact(
                self.source,
                ledger,
                start,
                HSACO_PROGRAM_HEADER_BYTES,
                HsacoEvidence::object(ByteRange { start, len: HSACO_PROGRAM_HEADER_BYTES }),
            )?;
            let offset = read_u64(&bytes, 8);
            let file_bytes = read_u64(&bytes, 32);
            let fits = offset.checked_add(file_bytes).is_some_and(|end| end <= self.length);
            if !fits {
                return Err(HsacoLayoutError::SegmentOutOfBounds {
                    segment_index: index as u32,
                    offset,
                    file_bytes,
                    object_bytes: self.length,
                    evidence: HsacoEvidence::object(ByteRange { start: offset, len: file_bytes }),
                });
            }
            segments.push(HsacoSegment {
                index: index as u32,
                segment_type: read_u32(&bytes, 0),
                flags: read_u32(&bytes, 4),
                byte_range: ByteRange { start: offset, len: file_bytes },
            });
        }
        Ok(segments)
    }

    /// Checks that every section payload fits inside the object.
    fn check_section_bounds(&self, headers: &[SectionHeader]) -> Result<(), HsacoLayoutError> {
        for (position, header) in headers.iter().enumerate() {
            if header.section_type == HSACO_SHT_NOBITS || header.size == 0 {
                continue;
            }
            let section_index = position as u32;
            let evidence = HsacoEvidence::section(section_index, ByteRange { start: header.offset, len: header.size });
            let fits = header.offset.checked_add(header.size).is_some_and(|end| end <= self.length);
            if !fits {
                return Err(HsacoLayoutError::SectionOutOfBounds {
                    section_index,
                    offset: header.offset,
                    size: header.size,
                    object_bytes: self.length,
                    evidence,
                });
            }
        }
        Ok(())
    }

    /// Checks that no two sections with file bytes claim overlapping intervals.
    fn check_section_overlap(&self, headers: &[SectionHeader]) -> Result<(), HsacoLayoutError> {
        let mut occupied: Vec<(u32, ByteRange)> = headers
            .iter()
            .enumerate()
            .filter(|(_, header)| header.section_type != HSACO_SHT_NOBITS && header.size != 0)
            .map(|(position, header)| (position as u32, header.byte_range()))
            .collect();
        occupied.sort_by_key(|(_, byte_range)| byte_range.start);
        for pair in occupied.windows(2) {
            let (first_index, first) = pair[0];
            let (second_index, second) = pair[1];
            let first_end = first.start + first.len;
            if first_end > second.start {
                return Err(HsacoLayoutError::SectionOverlap {
                    first_index,
                    second_index,
                    evidence: HsacoEvidence::section(second_index, second),
                });
            }
        }
        Ok(())
    }

    /// Resolves every section name through the section name table.
    fn section_names(&self, ledger: &mut BudgetTracker, headers: &[SectionHeader]) -> Result<Vec<String>, HsacoLayoutError> {
        if headers.is_empty() {
            return Ok(Vec::new());
        }

        let index = self.header.section_name_table_index;
        let Some(table) = headers.get(usize::from(index))
        else {
            return Err(HsacoLayoutError::SectionNameTableOutOfBounds {
                index,
                section_count: headers.len() as u64,
                evidence: HsacoEvidence::object(ByteRange { start: 62, len: 2 }),
            });
        };

        let len = if table.section_type == HSACO_SHT_NOBITS { 0 } else { table.size };
        let bytes = if len == 0 {
            Vec::new()
        }
        else {
            read_exact(
                self.source,
                ledger,
                table.offset,
                len,
                HsacoEvidence::section(index as u32, ByteRange { start: table.offset, len }),
            )?
        };

        Ok(headers.iter().map(|header| read_section_name(&bytes, header.name_offset)).collect())
    }

    /// Evidence for one field of a section header entry.
    fn section_field(&self, index: u32, field_offset: u64) -> HsacoEvidence {
        let entry = u64::from(self.header.section_header_entry_size);
        let start = self.header.section_header_offset + u64::from(index) * entry + field_offset;
        HsacoEvidence::section(index, ByteRange { start, len: 8 })
    }
}

/// Maps and validates an ELF64 code object in one step.
///
/// Returns the object summary for a structurally valid code object, or the
/// classified failure that was found. Instruction and metadata semantics are
/// never inspected.
pub fn validate_hsaco<S: ByteSource + ?Sized>(
    source: &S,
    ledger: &mut BudgetTracker,
) -> Result<HsacoSummary, HsacoLayoutError> {
    HsacoObject::open(source, ledger)?.validate(ledger)
}

/// One decoded section header entry.
#[derive(Debug, Clone, Copy)]
struct SectionHeader {
    name_offset: u32,
    section_type: u32,
    offset: u64,
    size: u64,
    entry_size: u64,
}

impl SectionHeader {
    /// File interval the section occupies, empty for sections without file bytes.
    fn byte_range(&self) -> ByteRange {
        let len = if self.section_type == HSACO_SHT_NOBITS { 0 } else { self.size };
        ByteRange { start: self.offset, len }
    }
}

/// Declared geometry of one header table.
#[derive(Debug, Clone, Copy)]
struct TableLayout {
    table: HsacoHeaderTable,
    offset: u64,
    count: u64,
    entry_bytes: u64,
    minimum_entry_bytes: u64,
    offset_evidence: HsacoEvidence,
    entry_size_evidence: HsacoEvidence,
}

/// Bounds one header table and reports the bytes it occupies.
///
/// A declared table that does not fit is reported as a partial trailing entry
/// when the remaining bytes stop inside one, and as an out-of-bounds table
/// otherwise.
fn table_bytes(length: u64, layout: TableLayout) -> Result<u64, HsacoLayoutError> {
    let TableLayout { table, offset, count, entry_bytes, minimum_entry_bytes, offset_evidence, entry_size_evidence } = layout;
    if count == 0 {
        return Ok(0);
    }
    if entry_bytes < minimum_entry_bytes {
        return Err(HsacoLayoutError::HeaderEntryTooSmall {
            table,
            entry_bytes,
            minimum: minimum_entry_bytes,
            evidence: entry_size_evidence,
        });
    }

    let table_bytes = count.checked_mul(entry_bytes).ok_or(HsacoLayoutError::HeaderTableOverflow {
        table,
        count,
        entry_bytes,
        evidence: offset_evidence,
    })?;

    if offset.checked_add(table_bytes).is_some_and(|end| end <= length) {
        return Ok(table_bytes);
    }

    let available_bytes = length.saturating_sub(offset);
    let complete = available_bytes / entry_bytes;
    let remainder = available_bytes % entry_bytes;
    if offset < length && complete < count && remainder != 0 {
        return Err(HsacoLayoutError::TrailingIncompleteEntry {
            table,
            offset: offset + complete * entry_bytes,
            available_bytes: remainder,
            entry_bytes,
            evidence: offset_evidence,
        });
    }

    Err(HsacoLayoutError::HeaderTableOutOfBounds {
        table,
        count,
        table_bytes,
        available_bytes,
        offset,
        evidence: offset_evidence,
    })
}

/// Resolves one null-terminated name inside the section name table.
fn read_section_name(table: &[u8], name_offset: u32) -> String {
    let start = name_offset as usize;
    if start >= table.len() {
        return String::new();
    }
    let rest = &table[start..];
    let end = rest.iter().position(|byte| *byte == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("2-byte header field"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("4-byte header field"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("8-byte header field"))
}

/// Reads exactly `len` bytes and settles the read against `ledger`.
pub(crate) fn read_exact<S: ByteSource + ?Sized>(
    source: &S,
    ledger: &mut BudgetTracker,
    start: u64,
    len: u64,
    evidence: HsacoEvidence,
) -> Result<Vec<u8>, HsacoLayoutError> {
    ledger.reserve_read_bytes(len).map_err(HsacoLayoutError::Budget)?;
    match source.read_range(SourceRange { start, len }) {
        Ok(bytes) => {
            let consumed = bytes.len() as u64;
            ledger.settle_read_bytes(consumed).map_err(HsacoLayoutError::Budget)?;
            if consumed < len {
                ledger.release_read_bytes(len - consumed).map_err(HsacoLayoutError::Budget)?;
                return Err(HsacoLayoutError::ShortRead { expected: len, actual: consumed, evidence });
            }
            Ok(bytes)
        }
        Err(ReadError::ShortRead { expected, actual }) => {
            let consumed = actual.min(len);
            ledger.settle_read_bytes(consumed).map_err(HsacoLayoutError::Budget)?;
            if consumed < len {
                ledger.release_read_bytes(len - consumed).map_err(HsacoLayoutError::Budget)?;
            }
            Err(HsacoLayoutError::ShortRead { expected, actual, evidence })
        }
        Err(error) => {
            ledger.release_read_bytes(len).map_err(HsacoLayoutError::Budget)?;
            Err(match error {
                ReadError::Unavailable { reason, .. } => HsacoLayoutError::Unavailable { reason, evidence },
                other => HsacoLayoutError::ReadFailed { message: other.to_string(), evidence },
            })
        }
    }
}
