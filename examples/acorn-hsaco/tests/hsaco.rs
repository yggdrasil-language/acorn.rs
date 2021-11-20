//! Structural tests for the ELF64 code object layout adapter.

use acorn_core::{Budget, BudgetError, BudgetTracker, ByteRange};
use acorn_hsaco::{
    HSACO_ELF_MAGIC, HSACO_EM_AMDGPU, HsacoByteError, HsacoHeaderTable, HsacoLayoutError, HsacoObject, HsacoSectionKind,
    validate_hsaco_bytes,
};
use acorn_source::{ByteSource, MemorySource, ReadError, ReadOutcome, ReadResult, SourceCapabilities, SourceIdentity};

const ELF_HEADER_BYTES: u64 = 64;
const PROGRAM_HEADER_BYTES: u64 = 56;
const SECTION_HEADER_BYTES: u64 = 64;
const SHT_NULL: u32 = 0;
const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const ENTRY_ADDRESS: u64 = 0x1000;

struct SectionSpec {
    name: &'static str,
    section_type: u32,
    payload: Vec<u8>,
    entry_size: u64,
}

fn section(name: &'static str, section_type: u32, payload: Vec<u8>) -> SectionSpec {
    SectionSpec { name, section_type, payload, entry_size: 0 }
}

fn push_name(table: &mut Vec<u8>, name: &str) -> u32 {
    let offset = u32::try_from(table.len()).expect("name table offset fits in a word");
    table.extend_from_slice(name.as_bytes());
    table.push(0);
    offset
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn align8(value: u64) -> u64 {
    value.div_ceil(8) * 8
}

/// Lays out an ELF64 code object: header, one `PT_LOAD` program header, the
/// section payloads, and the section header table with a trailing name table.
fn build_object(sections: &[SectionSpec]) -> Vec<u8> {
    let mut entries = vec![SectionSpec { name: "", section_type: SHT_NULL, payload: Vec::new(), entry_size: 0 }];
    entries.extend(sections.iter().map(|spec| SectionSpec {
        name: spec.name,
        section_type: spec.section_type,
        payload: spec.payload.clone(),
        entry_size: spec.entry_size,
    }));

    let mut names = Vec::new();
    let mut name_offsets = Vec::new();
    for entry in &entries {
        name_offsets.push(push_name(&mut names, entry.name));
    }
    let shstrtab_name = push_name(&mut names, ".shstrtab");
    entries.push(SectionSpec { name: ".shstrtab", section_type: SHT_STRTAB, payload: names, entry_size: 0 });
    name_offsets.push(shstrtab_name);

    let mut cursor = ELF_HEADER_BYTES + PROGRAM_HEADER_BYTES;
    let mut offsets = Vec::with_capacity(entries.len());
    for entry in &entries {
        if !entry.payload.is_empty() {
            offsets.push(cursor);
            cursor += entry.payload.len() as u64;
        }
        else {
            offsets.push(0);
        }
    }

    let section_table_offset = align8(cursor);
    let total = section_table_offset + SECTION_HEADER_BYTES * entries.len() as u64;
    let mut bytes = vec![0u8; total as usize];

    bytes[0..4].copy_from_slice(&HSACO_ELF_MAGIC);
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    put_u16(&mut bytes, 16, 2);
    put_u16(&mut bytes, 18, HSACO_EM_AMDGPU);
    put_u32(&mut bytes, 20, 1);
    put_u64(&mut bytes, 24, ENTRY_ADDRESS);
    put_u64(&mut bytes, 32, ELF_HEADER_BYTES);
    put_u64(&mut bytes, 40, section_table_offset);
    put_u32(&mut bytes, 48, 0);
    put_u16(&mut bytes, 52, 64);
    put_u16(&mut bytes, 54, 56);
    put_u16(&mut bytes, 56, 1);
    put_u16(&mut bytes, 58, 64);
    put_u16(&mut bytes, 60, u16::try_from(entries.len()).expect("section count fits in a word"));
    put_u16(&mut bytes, 62, u16::try_from(entries.len() - 1).expect("name table index fits in a word"));

    let text = entries
        .iter()
        .position(|entry| entry.name == ".text")
        .map(|position| (offsets[position], entries[position].payload.len() as u64));
    let (text_offset, text_size) = text.unwrap_or((0, 0));
    put_u32(&mut bytes, 64, 1);
    put_u32(&mut bytes, 68, 5);
    put_u64(&mut bytes, 72, text_offset);
    put_u64(&mut bytes, 96, text_size);

    for (position, entry) in entries.iter().enumerate() {
        if offsets[position] != 0 && !entry.payload.is_empty() {
            let start = offsets[position] as usize;
            bytes[start..start + entry.payload.len()].copy_from_slice(&entry.payload);
        }
        let base = (section_table_offset + SECTION_HEADER_BYTES * position as u64) as usize;
        put_u32(&mut bytes, base, name_offsets[position]);
        put_u32(&mut bytes, base + 4, entry.section_type);
        put_u64(&mut bytes, base + 24, offsets[position]);
        put_u64(&mut bytes, base + 32, entry.payload.len() as u64);
        put_u64(&mut bytes, base + 56, entry.entry_size);
    }

    bytes
}

fn minimal_sections() -> Vec<SectionSpec> {
    vec![section(".text", SHT_PROGBITS, vec![0xAA; 16])]
}

fn minimal_object() -> Vec<u8> {
    build_object(&minimal_sections())
}

fn full_sections() -> Vec<SectionSpec> {
    vec![
        section(".text", SHT_PROGBITS, vec![0xAA; 16]),
        section(".rodata", SHT_PROGBITS, vec![0xBB; 8]),
        section(".note", SHT_PROGBITS, vec![0xCC; 12]),
        section(".amdgpu_metadata", SHT_PROGBITS, vec![0xDD; 24]),
        SectionSpec { name: ".symtab", section_type: SHT_SYMTAB, payload: vec![0x00; 48], entry_size: 24 },
        section(".strtab", SHT_STRTAB, vec![0x00; 10]),
    ]
}

fn patch_u16(bytes: &mut [u8], offset: usize, value: u16) {
    put_u16(bytes, offset, value);
}

fn patch_u64(bytes: &mut [u8], offset: usize, value: u64) {
    put_u64(bytes, offset, value);
}

fn memory_source(bytes: Vec<u8>) -> MemorySource {
    MemorySource::from_bytes("code.hsaco", bytes)
}

#[test]
fn parses_a_minimal_object_with_text_and_its_name_table() {
    let summary = validate_hsaco_bytes(&minimal_object()).expect("minimal object is structurally valid");

    assert_eq!(summary.header.machine, HSACO_EM_AMDGPU);
    assert_eq!(summary.header.object_type, 2);
    assert_eq!(summary.header.entry, ENTRY_ADDRESS);
    assert_eq!(summary.header.ident_version, 1);
    assert_eq!(summary.header.version, 1);
    assert_eq!(summary.header.flags, 0);
    assert_eq!(summary.header.program_header_count, 1);
    assert_eq!(summary.header.section_header_count, 3);
    assert_eq!(summary.header.section_name_table_index, 2);
    assert_eq!(summary.header.section_header_offset, 160);

    assert_eq!(summary.section_count, 3);
    assert_eq!(summary.text, ByteRange { start: 120, len: 16 });
    assert_eq!(summary.symbol_count, None);
    assert_eq!(summary.amdgpu_metadata, None);

    assert_eq!(
        summary.sections.iter().map(|section| section.kind).collect::<Vec<_>>(),
        vec![HsacoSectionKind::Other, HsacoSectionKind::Text, HsacoSectionKind::StringTable]
    );
    assert_eq!(summary.sections[1].name, ".text");
    assert_eq!(summary.sections[1].byte_range, ByteRange { start: 120, len: 16 });
    assert_eq!(summary.sections[2].name, ".shstrtab");

    assert_eq!(summary.segments.len(), 1);
    assert_eq!(summary.segments[0].index, 0);
    assert_eq!(summary.segments[0].segment_type, 1);
    assert_eq!(summary.segments[0].flags, 5);
    assert_eq!(summary.segments[0].byte_range, ByteRange { start: 120, len: 16 });
}

#[test]
fn object_view_matches_the_byte_slice_entry() {
    let bytes = minimal_object();
    let source = memory_source(bytes.clone());
    let mut ledger = BudgetTracker::new(Budget::default());

    let object = HsacoObject::open(&source, &mut ledger).expect("the header and tables fit");
    assert_eq!(object.header().machine, HSACO_EM_AMDGPU);
    assert_eq!(object.header().section_header_count, 3);
    let summary = object.validate(&mut ledger).expect("validate the object");

    assert_eq!(summary, validate_hsaco_bytes(&bytes).expect("byte slice entry agrees"));
}

#[test]
fn records_every_key_section_of_a_full_object() {
    let summary = validate_hsaco_bytes(&build_object(&full_sections())).expect("full object is structurally valid");

    assert_eq!(summary.section_count, 8);
    assert_eq!(summary.text, ByteRange { start: 120, len: 16 });
    assert_eq!(summary.symbol_count, Some(2));
    assert_eq!(summary.amdgpu_metadata, Some(ByteRange { start: 156, len: 24 }));
    assert_eq!(summary.header.section_header_offset, 304);

    assert_eq!(
        summary.sections.iter().map(|section| section.kind).collect::<Vec<_>>(),
        vec![
            HsacoSectionKind::Other,
            HsacoSectionKind::Text,
            HsacoSectionKind::Rodata,
            HsacoSectionKind::Note,
            HsacoSectionKind::AmdgpuMetadata,
            HsacoSectionKind::SymbolTable,
            HsacoSectionKind::StringTable,
            HsacoSectionKind::StringTable,
        ]
    );
    assert_eq!(summary.sections[2].byte_range, ByteRange { start: 136, len: 8 });
    assert_eq!(summary.sections[3].byte_range, ByteRange { start: 144, len: 12 });
    assert_eq!(summary.sections[5].byte_range, ByteRange { start: 180, len: 48 });
    assert_eq!(summary.segments[0].byte_range, ByteRange { start: 120, len: 16 });
}

#[test]
fn bad_magic_keeps_the_magic_range() {
    let mut bytes = minimal_object();
    bytes[0..4].copy_from_slice(&[0x7F, b'B', b'D', b'X']);

    let error = validate_hsaco_bytes(&bytes).expect_err("bad magic must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::BadMagic { magic, evidence }) => {
            assert_eq!(magic, [0x7F, b'B', b'D', b'X']);
            assert_eq!(evidence.section_index, None);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn class_must_be_64_bit() {
    let mut bytes = minimal_object();
    bytes[4] = 1;

    let error = validate_hsaco_bytes(&bytes).expect_err("a 32-bit class must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::UnsupportedClass { class, evidence }) => {
            assert_eq!(class, 1);
            assert_eq!(evidence.byte_range, ByteRange { start: 4, len: 1 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn data_encoding_must_be_little_endian() {
    let mut bytes = minimal_object();
    bytes[5] = 2;

    let error = validate_hsaco_bytes(&bytes).expect_err("a big endian object must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::UnsupportedEndianness { data, evidence }) => {
            assert_eq!(data, 2);
            assert_eq!(evidence.byte_range, ByteRange { start: 5, len: 1 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn machine_must_be_amdgpu() {
    let mut bytes = minimal_object();
    patch_u16(&mut bytes, 18, 62);

    let error = validate_hsaco_bytes(&bytes).expect_err("a host machine must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::MachineMismatch { machine, evidence }) => {
            assert_eq!(machine, 62);
            assert_eq!(evidence.byte_range, ByteRange { start: 18, len: 2 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_entry_rejects_a_truncated_header() {
    let bytes = minimal_object()[..32].to_vec();

    let error = validate_hsaco_bytes(&bytes).expect_err("a truncated header must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::TruncatedHeader { available_bytes, evidence }) => {
            assert_eq!(available_bytes, 32);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 32 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn section_header_table_past_the_object_is_rejected() {
    let mut bytes = minimal_object();
    patch_u64(&mut bytes, 40, 1000);

    let error = validate_hsaco_bytes(&bytes).expect_err("an offset past the object must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::HeaderTableOutOfBounds {
            table,
            count,
            table_bytes,
            available_bytes,
            offset,
            evidence,
        }) => {
            assert_eq!(table, HsacoHeaderTable::Section);
            assert_eq!(count, 3);
            assert_eq!(table_bytes, 192);
            assert_eq!(available_bytes, 0);
            assert_eq!(offset, 1000);
            assert_eq!(evidence.byte_range, ByteRange { start: 40, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn program_header_table_past_the_object_is_rejected() {
    let mut bytes = minimal_object();
    patch_u64(&mut bytes, 32, 1000);

    let error = validate_hsaco_bytes(&bytes).expect_err("a program table past the object must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::HeaderTableOutOfBounds {
            table,
            count,
            table_bytes,
            available_bytes,
            offset,
            evidence,
        }) => {
            assert_eq!(table, HsacoHeaderTable::Program);
            assert_eq!(count, 1);
            assert_eq!(table_bytes, 56);
            assert_eq!(available_bytes, 0);
            assert_eq!(offset, 1000);
            assert_eq!(evidence.byte_range, ByteRange { start: 32, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn section_header_entry_size_below_the_minimum_is_rejected() {
    let mut bytes = minimal_object();
    patch_u16(&mut bytes, 58, 32);

    let error = validate_hsaco_bytes(&bytes).expect_err("a 32-byte section entry must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::HeaderEntryTooSmall { table, entry_bytes, minimum, evidence }) => {
            assert_eq!(table, HsacoHeaderTable::Section);
            assert_eq!(entry_bytes, 32);
            assert_eq!(minimum, 64);
            assert_eq!(evidence.byte_range, ByteRange { start: 58, len: 2 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn declared_section_count_past_the_object_is_rejected() {
    let mut bytes = minimal_object();
    patch_u16(&mut bytes, 60, 4096);

    let error = validate_hsaco_bytes(&bytes).expect_err("an oversized table must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::HeaderTableOutOfBounds {
            table,
            count,
            table_bytes,
            available_bytes,
            offset,
            evidence,
        }) => {
            assert_eq!(table, HsacoHeaderTable::Section);
            assert_eq!(count, 4096);
            assert_eq!(table_bytes, 4096 * 64);
            assert_eq!(available_bytes, 192);
            assert_eq!(offset, 160);
            assert_eq!(evidence.byte_range, ByteRange { start: 40, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn trailing_incomplete_section_header_entry_is_rejected() {
    let mut bytes = minimal_object();
    bytes.truncate(342);

    let error = validate_hsaco_bytes(&bytes).expect_err("a partial trailing entry must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::TrailingIncompleteEntry {
            table,
            offset,
            available_bytes,
            entry_bytes,
            evidence,
        }) => {
            assert_eq!(table, HsacoHeaderTable::Section);
            assert_eq!(offset, 288);
            assert_eq!(available_bytes, 54);
            assert_eq!(entry_bytes, 64);
            assert_eq!(evidence.byte_range, ByteRange { start: 40, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn trailing_incomplete_program_header_entry_is_rejected() {
    let mut bytes = minimal_object();
    patch_u64(&mut bytes, 32, 322);

    let error = validate_hsaco_bytes(&bytes).expect_err("a partial trailing entry must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::TrailingIncompleteEntry {
            table,
            offset,
            available_bytes,
            entry_bytes,
            evidence,
        }) => {
            assert_eq!(table, HsacoHeaderTable::Program);
            assert_eq!(offset, 322);
            assert_eq!(available_bytes, 30);
            assert_eq!(entry_bytes, 56);
            assert_eq!(evidence.byte_range, ByteRange { start: 32, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn section_payload_past_the_object_is_rejected() {
    let mut bytes = minimal_object();
    // Section header 1 is `.text`, so its size word is at 160 + 64 + 32.
    patch_u64(&mut bytes, 256, 100_000);

    let error = validate_hsaco_bytes(&bytes).expect_err("a section past the object must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::SectionOutOfBounds {
            section_index,
            offset,
            size,
            object_bytes,
            evidence,
        }) => {
            assert_eq!(section_index, 1);
            assert_eq!(offset, 120);
            assert_eq!(size, 100_000);
            assert_eq!(object_bytes, 352);
            assert_eq!(evidence.section_index, Some(1));
            assert_eq!(evidence.byte_range, ByteRange { start: 120, len: 100_000 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn segment_payload_past_the_object_is_rejected() {
    let mut bytes = minimal_object();
    patch_u64(&mut bytes, 96, 100_000);

    let error = validate_hsaco_bytes(&bytes).expect_err("a segment past the object must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::SegmentOutOfBounds {
            segment_index,
            offset,
            file_bytes,
            object_bytes,
            evidence,
        }) => {
            assert_eq!(segment_index, 0);
            assert_eq!(offset, 120);
            assert_eq!(file_bytes, 100_000);
            assert_eq!(object_bytes, 352);
            assert_eq!(evidence.section_index, None);
            assert_eq!(evidence.byte_range, ByteRange { start: 120, len: 100_000 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn overlapping_sections_are_rejected() {
    let mut bytes = minimal_object();
    // Point `.shstrtab` at the `.text` payload.
    patch_u64(&mut bytes, 312, 120);

    let error = validate_hsaco_bytes(&bytes).expect_err("overlapping sections must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::SectionOverlap { first_index, second_index, evidence }) => {
            assert_eq!(first_index, 1);
            assert_eq!(second_index, 2);
            assert_eq!(evidence.section_index, Some(2));
            assert_eq!(evidence.byte_range, ByteRange { start: 120, len: 17 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn object_without_a_text_section_is_rejected() {
    let bytes = build_object(&[section(".data", SHT_PROGBITS, vec![0xEE; 8])]);

    let error = validate_hsaco_bytes(&bytes).expect_err("a missing `.text` must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::MissingTextSection { evidence }) => {
            assert_eq!(evidence.section_index, None);
            assert_eq!(evidence.byte_range, ByteRange { start: 40, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn empty_text_section_is_rejected() {
    let bytes = build_object(&[section(".text", SHT_PROGBITS, Vec::new())]);

    let error = validate_hsaco_bytes(&bytes).expect_err("an empty `.text` must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::EmptyTextSection { section_index, evidence }) => {
            assert_eq!(section_index, 1);
            assert_eq!(evidence.section_index, Some(1));
            assert_eq!(evidence.byte_range, ByteRange { start: 240, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn section_name_table_index_must_address_a_section() {
    let mut bytes = minimal_object();
    patch_u16(&mut bytes, 62, 9);

    let error = validate_hsaco_bytes(&bytes).expect_err("a dangling name table index must fail");
    match error {
        HsacoByteError::Layout(HsacoLayoutError::SectionNameTableOutOfBounds { index, section_count, evidence }) => {
            assert_eq!(index, 9);
            assert_eq!(section_count, 3);
            assert_eq!(evidence.byte_range, ByteRange { start: 62, len: 2 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_error_keeps_the_original_error() {
    let mut bytes = minimal_object();
    bytes[0..4].copy_from_slice(&[0x7F, b'B', b'D', b'X']);

    let error = validate_hsaco_bytes(&bytes).expect_err("bad magic must fail");
    assert!(error.to_string().contains("magic"), "{error}");
    assert!(std::error::Error::source(&error).is_some(), "the typed cause must be preserved");
}

#[test]
fn byte_error_maps_layout_budget_and_source_failures() {
    assert_eq!(HsacoByteError::from(HsacoLayoutError::UnknownLength), HsacoByteError::Layout(HsacoLayoutError::UnknownLength));
    assert_eq!(
        HsacoByteError::from(HsacoLayoutError::Budget(BudgetError::ReadBytes)),
        HsacoByteError::Budget(BudgetError::ReadBytes)
    );
    assert_eq!(HsacoByteError::from(BudgetError::ScanBytes), HsacoByteError::Budget(BudgetError::ScanBytes));
    assert_eq!(HsacoByteError::from(ReadError::Eof), HsacoByteError::Source(ReadError::Eof));
}

#[test]
fn budget_is_checked_before_the_header_is_read() {
    let source = memory_source(minimal_object());
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: 32, ..Budget::default() });

    let error = acorn_hsaco::validate_hsaco(&source, &mut ledger).expect_err("a small read budget must fail");
    assert_eq!(error, HsacoLayoutError::Budget(BudgetError::ReadBytes));
}

#[test]
fn budget_is_checked_while_section_headers_are_read() {
    let source = memory_source(minimal_object());
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: 184, ..Budget::default() });

    let object = HsacoObject::open(&source, &mut ledger).expect("the header fits the allowance");
    let error = object.validate(&mut ledger).expect_err("the third section header exceeds the allowance");
    assert_eq!(error, HsacoLayoutError::Budget(BudgetError::ReadBytes));
}

#[test]
fn short_header_read_keeps_the_requested_range() {
    #[derive(Debug)]
    struct ShortSource {
        bytes: Vec<u8>,
        declared_len: u64,
    }

    impl ByteSource for ShortSource {
        fn length(&self) -> Option<u64> {
            Some(self.declared_len)
        }

        fn read_at(&self, offset: u64, destination: &mut [u8]) -> ReadResult {
            let offset = offset as usize;
            if offset >= self.bytes.len() {
                return Ok(ReadOutcome::Eof);
            }
            let available = self.bytes.len() - offset;
            let to_copy = destination.len().min(available);
            destination[..to_copy].copy_from_slice(&self.bytes[offset..offset + to_copy]);
            if to_copy == destination.len() {
                Ok(ReadOutcome::Complete)
            }
            else {
                Ok(ReadOutcome::Partial { bytes_read: to_copy as u64 })
            }
        }

        fn capabilities(&self) -> SourceCapabilities {
            SourceCapabilities::PARTIAL
        }

        fn identity(&self) -> SourceIdentity {
            SourceIdentity::new("short")
        }
    }

    let source = ShortSource { bytes: vec![0u8; 40], declared_len: 352 };
    let mut ledger = BudgetTracker::new(Budget::default());
    let error = HsacoObject::open(&source, &mut ledger).expect_err("a short header read must fail");
    match error {
        HsacoLayoutError::ShortRead { expected, actual, evidence } => {
            assert_eq!(expected, 64);
            assert_eq!(actual, 40);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 64 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
