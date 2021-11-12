//! Structural tests for the SPIR-V module layout adapter.

use acorn_core::{Budget, BudgetError, BudgetTracker, ByteRange};
use acorn_source::{ByteSource, MemorySource, ReadError, ReadOutcome, ReadResult, SourceCapabilities, SourceIdentity};
use acorn_spirv::{
    SPIRV_ID_BOUND_CEILING, SPIRV_MAGIC, SpirvBoundSuspicion, SpirvByteError, SpirvLayoutError, SpirvModule, validate_spirv,
    validate_spirv_bytes,
};

const OP_CAPABILITY: u16 = 17;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;

const VERSION_1_0: u32 = 0x0001_0000;
const GENERATOR: u32 = 0x0012_0001;

fn push_word(bytes: &mut Vec<u8>, word: u32) {
    bytes.extend_from_slice(&word.to_le_bytes());
}

fn push_instruction(bytes: &mut Vec<u8>, opcode: u16, operands: &[u32]) {
    let word_count = 1 + u32::try_from(operands.len()).expect("operand count fits in a word");
    push_word(bytes, (word_count << 16) | u32::from(opcode));
    for operand in operands {
        push_word(bytes, *operand);
    }
}

fn push_header(bytes: &mut Vec<u8>, magic: u32, bound: u32, schema: u32) {
    push_word(bytes, magic);
    push_word(bytes, VERSION_1_0);
    push_word(bytes, GENERATOR);
    push_word(bytes, bound);
    push_word(bytes, schema);
}

fn push_minimal_body(bytes: &mut Vec<u8>) {
    // OpCapability Shader
    push_instruction(bytes, OP_CAPABILITY, &[1]);
    // OpMemoryModel Logical GLSL450
    push_instruction(bytes, OP_MEMORY_MODEL, &[0, 1]);
    // OpEntryPoint GLCompute %main "main", with the name as one padded word.
    push_instruction(bytes, OP_ENTRY_POINT, &[5, 4, 0x6E69_616D, 0]);
}

fn minimal_module() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, 16, 0);
    push_minimal_body(&mut bytes);
    bytes
}

fn module_with_bound(bound: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, bound, 0);
    push_minimal_body(&mut bytes);
    bytes
}

fn memory_source(label: &str, bytes: Vec<u8>) -> MemorySource {
    MemorySource::from_bytes(label, bytes)
}

#[test]
fn parses_minimal_module_with_instruction_ranges() {
    let source = memory_source("minimal.spv", minimal_module());
    let mut ledger = BudgetTracker::new(Budget::default());

    let module = SpirvModule::open(&source, &mut ledger).expect("open minimal module");
    assert_eq!(module.header().magic, SPIRV_MAGIC);
    assert_eq!(module.header().version, VERSION_1_0);
    assert_eq!(module.header().generator, GENERATOR);
    assert_eq!(module.header().bound, 16);
    assert_eq!(module.header().schema, 0);
    assert_eq!(module.instruction_words(), 10);
    assert_eq!(module.total_words(), 15);

    let instructions = module.instructions(&mut ledger).map(|item| item.expect("instruction layout")).collect::<Vec<_>>();
    assert_eq!(instructions.len(), 3);
    assert_eq!(
        instructions[0],
        acorn_spirv::SpirvInstruction {
            opcode: OP_CAPABILITY,
            word_count: 2,
            word_offset: 5,
            byte_range: ByteRange { start: 20, len: 8 },
        }
    );
    assert_eq!(instructions[1].opcode, OP_MEMORY_MODEL);
    assert_eq!(instructions[1].word_count, 3);
    assert_eq!(instructions[1].word_offset, 7);
    assert_eq!(instructions[1].byte_range, ByteRange { start: 28, len: 12 });
    assert_eq!(instructions[2].opcode, OP_ENTRY_POINT);
    assert_eq!(instructions[2].word_count, 5);
    assert_eq!(instructions[2].word_offset, 10);
    assert_eq!(instructions[2].byte_range, ByteRange { start: 40, len: 20 });

    let summary = module.validate(&mut ledger).expect("validate minimal module");
    assert_eq!(summary.header, module.header());
    assert_eq!(summary.instruction_count, 3);
    assert_eq!(summary.instruction_words, 10);
    assert_eq!(summary.total_words, 15);
    assert_eq!(summary.bound_suspicion, None);

    let direct = validate_spirv(&source, &mut ledger).expect("validate entry");
    assert_eq!(direct, summary);
}

#[test]
fn bad_magic_keeps_the_magic_word_range() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, 0xDEAD_BEEF, 16, 0);
    push_minimal_body(&mut bytes);
    let source = memory_source("bad-magic.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let error = SpirvModule::open(&source, &mut ledger).expect_err("bad magic must fail");
    match error {
        SpirvLayoutError::BadMagic { magic, evidence } => {
            assert_eq!(magic, 0xDEAD_BEEF);
            assert_eq!(evidence.word_offset, 0);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 4 });
            assert_eq!(evidence.word_count(), 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn non_zero_schema_keeps_the_schema_word_range() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, 16, 7);
    push_minimal_body(&mut bytes);
    let source = memory_source("schema.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let error = SpirvModule::open(&source, &mut ledger).expect_err("non-zero schema must fail");
    match error {
        SpirvLayoutError::NonZeroSchema { schema, evidence } => {
            assert_eq!(schema, 7);
            assert_eq!(evidence.word_offset, 4);
            assert_eq!(evidence.byte_range, ByteRange { start: 16, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn truncated_header_reports_available_bytes() {
    let mut bytes = Vec::new();
    push_word(&mut bytes, SPIRV_MAGIC);
    push_word(&mut bytes, VERSION_1_0);
    push_word(&mut bytes, GENERATOR);
    let source = memory_source("short-header.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let error = SpirvModule::open(&source, &mut ledger).expect_err("short header must fail");
    match error {
        SpirvLayoutError::TruncatedHeader { available_bytes, evidence } => {
            assert_eq!(available_bytes, 12);
            assert_eq!(evidence.word_offset, 0);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 12 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn zero_word_count_keeps_the_instruction_word_range() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, 16, 0);
    push_word(&mut bytes, u32::from(OP_CAPABILITY));
    let source = memory_source("zero-word-count.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let module = SpirvModule::open(&source, &mut ledger).expect("header and stream geometry are valid");
    let error = module.validate(&mut ledger).expect_err("zero word count must fail");
    match error {
        SpirvLayoutError::ZeroWordCount { evidence } => {
            assert_eq!(evidence.word_offset, 5);
            assert_eq!(evidence.byte_range, ByteRange { start: 20, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn instruction_past_the_stream_end_is_rejected() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, 16, 0);
    push_word(&mut bytes, (2 << 16) | u32::from(OP_CAPABILITY));
    let source = memory_source("out-of-bounds.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let module = SpirvModule::open(&source, &mut ledger).expect("header and stream geometry are valid");
    let error = module.validate(&mut ledger).expect_err("declared length past the end must fail");
    match error {
        SpirvLayoutError::InstructionOutOfBounds { word_count, available_words, evidence } => {
            assert_eq!(word_count, 2);
            assert_eq!(available_words, 1);
            assert_eq!(evidence.word_offset, 5);
            assert_eq!(evidence.byte_range, ByteRange { start: 20, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn trailing_partial_word_is_rejected() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, 16, 0);
    push_instruction(&mut bytes, OP_CAPABILITY, &[1]);
    bytes.extend_from_slice(&[0xAA, 0xBB]);
    let source = memory_source("partial-word.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let error = SpirvModule::open(&source, &mut ledger).expect_err("partial trailing word must fail");
    match error {
        SpirvLayoutError::TrailingPartialWord { remainder_bytes, evidence } => {
            assert_eq!(remainder_bytes, 2);
            assert_eq!(evidence.word_offset, 7);
            assert_eq!(evidence.byte_range, ByteRange { start: 28, len: 2 });
            assert_eq!(evidence.word_count(), 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn empty_instruction_stream_is_rejected() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, SPIRV_MAGIC, 16, 0);
    let source = memory_source("header-only.spv", bytes);
    let mut ledger = BudgetTracker::new(Budget::default());

    let error = SpirvModule::open(&source, &mut ledger).expect_err("a header without instructions must fail");
    match error {
        SpirvLayoutError::EmptyInstructionStream { evidence } => {
            assert_eq!(evidence.word_offset, 5);
            assert_eq!(evidence.byte_range, ByteRange { start: 20, len: 0 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn budget_is_checked_before_the_header_is_read() {
    let source = memory_source("budget-header.spv", minimal_module());
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: 8, ..Budget::default() });

    let error = SpirvModule::open(&source, &mut ledger).expect_err("a small read budget must fail");
    assert_eq!(error, SpirvLayoutError::Budget(BudgetError::ReadBytes));
}

#[test]
fn budget_is_checked_while_instructions_are_read() {
    let source = memory_source("budget-stream.spv", minimal_module());
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: 24, ..Budget::default() });

    let module = SpirvModule::open(&source, &mut ledger).expect("the header fits the allowance");
    let error = module.validate(&mut ledger).expect_err("the second instruction word exceeds the allowance");
    assert_eq!(error, SpirvLayoutError::Budget(BudgetError::ReadBytes));
}

#[test]
fn zero_bound_is_reported_as_suspicious_without_rejecting() {
    let source = memory_source("zero-bound.spv", module_with_bound(0));
    let mut ledger = BudgetTracker::new(Budget::default());

    let summary = validate_spirv(&source, &mut ledger).expect("a zero bound is still structurally valid");
    assert_eq!(summary.header.bound, 0);
    assert_eq!(summary.bound_suspicion, Some(SpirvBoundSuspicion::Zero));
}

#[test]
fn bound_above_the_universal_ceiling_is_reported_as_suspicious() {
    let bound = SPIRV_ID_BOUND_CEILING + 1;
    let source = memory_source("large-bound.spv", module_with_bound(bound));
    let mut ledger = BudgetTracker::new(Budget::default());

    let summary = validate_spirv(&source, &mut ledger).expect("a large bound is still structurally valid");
    assert_eq!(summary.bound_suspicion, Some(SpirvBoundSuspicion::AboveCeiling { bound, ceiling: SPIRV_ID_BOUND_CEILING }));
}

#[test]
fn source_without_a_length_is_reported() {
    #[derive(Debug)]
    struct LengthlessSource;

    impl ByteSource for LengthlessSource {
        fn length(&self) -> Option<u64> {
            None
        }

        fn read_at(&self, _offset: u64, _destination: &mut [u8]) -> ReadResult {
            Ok(ReadOutcome::Eof)
        }

        fn capabilities(&self) -> SourceCapabilities {
            SourceCapabilities::PARTIAL
        }

        fn identity(&self) -> SourceIdentity {
            SourceIdentity::new("lengthless")
        }
    }

    let mut ledger = BudgetTracker::new(Budget::default());
    let error = SpirvModule::open(&LengthlessSource, &mut ledger).expect_err("a missing length must fail");
    assert_eq!(error, SpirvLayoutError::UnknownLength);
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

    let source = ShortSource { bytes: vec![0u8; 12], declared_len: 24 };
    let mut ledger = BudgetTracker::new(Budget::default());
    let error = SpirvModule::open(&source, &mut ledger).expect_err("a short header read must fail");
    match error {
        SpirvLayoutError::ShortRead { expected, actual, evidence } => {
            assert_eq!(expected, 20);
            assert_eq!(actual, 12);
            assert_eq!(evidence.word_offset, 0);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 20 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_entry_validates_minimal_module() {
    let summary = validate_spirv_bytes(&minimal_module()).expect("minimal module is structurally valid");
    assert_eq!(summary.header.magic, SPIRV_MAGIC);
    assert_eq!(summary.header.version, VERSION_1_0);
    assert_eq!(summary.header.bound, 16);
    assert_eq!(summary.instruction_count, 3);
    assert_eq!(summary.instruction_words, 10);
    assert_eq!(summary.total_words, 15);
    assert_eq!(summary.bound_suspicion, None);
}

#[test]
fn byte_slice_entry_rejects_bad_magic() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, 0xDEAD_BEEF, 16, 0);
    push_minimal_body(&mut bytes);

    let error = validate_spirv_bytes(&bytes).expect_err("bad magic must fail");
    match error {
        SpirvByteError::Layout(SpirvLayoutError::BadMagic { magic, evidence }) => {
            assert_eq!(magic, 0xDEAD_BEEF);
            assert_eq!(evidence.word_offset, 0);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_entry_rejects_truncated_header() {
    let mut bytes = Vec::new();
    push_word(&mut bytes, SPIRV_MAGIC);
    push_word(&mut bytes, VERSION_1_0);
    push_word(&mut bytes, GENERATOR);

    let error = validate_spirv_bytes(&bytes).expect_err("a truncated header must fail");
    match error {
        SpirvByteError::Layout(SpirvLayoutError::TruncatedHeader { available_bytes, evidence }) => {
            assert_eq!(available_bytes, 12);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 12 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_error_keeps_the_original_error() {
    let mut bytes = Vec::new();
    push_header(&mut bytes, 0xDEAD_BEEF, 16, 0);

    let error = validate_spirv_bytes(&bytes).expect_err("bad magic must fail");
    assert!(error.to_string().contains("magic"), "{error}");
    assert!(std::error::Error::source(&error).is_some(), "the typed cause must be preserved");
}

#[test]
fn byte_error_maps_layout_budget_and_source_failures() {
    assert_eq!(SpirvByteError::from(SpirvLayoutError::UnknownLength), SpirvByteError::Layout(SpirvLayoutError::UnknownLength));
    assert_eq!(
        SpirvByteError::from(SpirvLayoutError::Budget(BudgetError::ReadBytes)),
        SpirvByteError::Budget(BudgetError::ReadBytes)
    );
    assert_eq!(SpirvByteError::from(BudgetError::ScanBytes), SpirvByteError::Budget(BudgetError::ScanBytes));
    assert_eq!(SpirvByteError::from(ReadError::Eof), SpirvByteError::Source(ReadError::Eof));
}
