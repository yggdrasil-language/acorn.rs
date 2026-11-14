//! Structural tests for the DXBC container layout adapter.

use acorn_core::{Budget, BudgetError, BudgetTracker, ByteRange};
use acorn_dxbc::{
    DXBC_MAGIC, DxbcByteError, DxbcChunkKind, DxbcContainer, DxbcInstruction, DxbcLayoutError, ShexProgram, validate_dxbc,
    validate_dxbc_bytes,
};
use acorn_source::MemorySource;

const DIGEST: [u8; 16] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00];
const CONTAINER_VERSION: u32 = 1;
const SHEX_VERSION_TOKEN: u32 = 0x0000_0040;
const OPCODE_DECLARE: u16 = 0x0001;
const OPCODE_RETURN: u16 = 0x003E;

/// Builds an SM5 opcode token: length in bits[30:24], opcode in bits[10:0].
fn instruction_token(opcode: u16, word_count: u32) -> u32 {
    (word_count << 24) | u32::from(opcode)
}

struct ChunkSpec {
    four_cc: [u8; 4],
    payload: Vec<u8>,
}

fn chunk(four_cc: [u8; 4], payload: Vec<u8>) -> ChunkSpec {
    ChunkSpec { four_cc, payload }
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

/// Builds one `SHEX` payload from its tokens and instruction words.
fn shex_payload(version_token: u32, length_token: u32, words: &[u32]) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u32(&mut payload, version_token);
    push_u32(&mut payload, length_token);
    for word in words {
        push_u32(&mut payload, *word);
    }
    payload
}

/// Two instructions: `OPCODE_DECLARE` with one operand word and `OPCODE_RETURN`.
///
/// The payload is 20 bytes, so the DWORD-counting length token is 5.
fn minimal_shader_payload() -> Vec<u8> {
    let words = [instruction_token(OPCODE_DECLARE, 2), 0x0000_0001, instruction_token(OPCODE_RETURN, 1)];
    shex_payload(SHEX_VERSION_TOKEN, 5, &words)
}

/// Raw `SHEX` payload emitted by the shader emitter for its `dispatch_fill` program.
///
/// The emitted container was accepted by a real D3D12 device for both PSO
/// creation and dispatch, so these bytes are a known-good SM5 stream: 52 DWORDs
/// of payload, a version token of `0x00050051`, and a first instruction token of
/// `0x0100086a` (`dcl_globalFlags`, length 1).
const EMITTED_FILL_SHEX: [u8; 208] = [
    0x51, 0x00, 0x05, 0x00, 0x34, 0x00, 0x00, 0x00, 0x6a, 0x08, 0x00, 0x01, //
    0x59, 0x00, 0x00, 0x07, 0x46, 0x8e, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x9d, 0x00, 0x00, 0x06, 0x46, 0xee, 0x31, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x5f, 0x00, 0x00, 0x02, 0x12, 0x00, 0x02, 0x00, //
    0x68, 0x00, 0x00, 0x02, 0x01, 0x00, 0x00, 0x00, 0x9b, 0x00, 0x00, 0x04, //
    0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, //
    0x50, 0x00, 0x00, 0x08, 0x12, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x0a, 0x00, 0x02, 0x00, 0x0a, 0x80, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f, 0x00, 0x04, 0x03, //
    0x0a, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3e, 0x00, 0x00, 0x01, //
    0x15, 0x00, 0x00, 0x01, 0x29, 0x00, 0x00, 0x06, 0x12, 0x00, 0x10, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x02, 0x00, 0x01, 0x40, 0x00, 0x00, //
    0x02, 0x00, 0x00, 0x00, 0xa6, 0x00, 0x00, 0x08, 0x12, 0xe0, 0x21, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x10, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x01, 0x40, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, //
    0x3e, 0x00, 0x00, 0x01,
];

fn minimal_chunks() -> Vec<ChunkSpec> {
    vec![
        chunk(*b"RDEF", vec![0x01, 0x02, 0x03, 0x04]),
        chunk(*b"ISGN", vec![0x00; 8]),
        chunk(*b"OSGN", vec![0x00; 8]),
        chunk(*b"SHEX", minimal_shader_payload()),
        chunk(*b"STAT", vec![0x00; 4]),
    ]
}

/// Lays out a container with the given chunks in table order.
fn build_container(version: u32, chunks: &[ChunkSpec]) -> Vec<u8> {
    let table_bytes = 4 * chunks.len();
    let mut total = 32 + table_bytes;
    let mut offsets = Vec::with_capacity(chunks.len());
    for spec in chunks {
        offsets.push(u32::try_from(total).expect("offset fits in a word"));
        total += 8 + spec.payload.len();
    }

    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&DXBC_MAGIC);
    bytes.extend_from_slice(&DIGEST);
    push_u32(&mut bytes, version);
    push_u32(&mut bytes, u32::try_from(total).expect("container length fits in a word"));
    push_u32(&mut bytes, u32::try_from(chunks.len()).expect("chunk count fits in a word"));
    for offset in &offsets {
        push_u32(&mut bytes, *offset);
    }
    for spec in chunks {
        bytes.extend_from_slice(&spec.four_cc);
        push_u32(&mut bytes, u32::try_from(spec.payload.len()).expect("payload length fits in a word"));
        bytes.extend_from_slice(&spec.payload);
    }
    bytes
}

fn minimal_container() -> Vec<u8> {
    build_container(CONTAINER_VERSION, &minimal_chunks())
}

fn patch_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn memory_source(bytes: Vec<u8>) -> MemorySource {
    MemorySource::from_bytes("container.dxbc", bytes)
}

#[test]
fn parses_minimal_container_with_every_common_chunk() {
    let summary = validate_dxbc_bytes(&minimal_container()).expect("minimal container is structurally valid");

    assert_eq!(summary.header.magic, DXBC_MAGIC);
    assert_eq!(summary.header.digest, DIGEST);
    assert_eq!(summary.header.version, CONTAINER_VERSION);
    assert_eq!(summary.header.total_length, 136);
    assert_eq!(summary.header.chunk_count, 5);

    assert_eq!(summary.chunks.len(), 5);
    assert_eq!(
        summary.chunks.iter().map(|chunk| chunk.kind).collect::<Vec<_>>(),
        vec![
            DxbcChunkKind::ResourceDefinition,
            DxbcChunkKind::InputSignature,
            DxbcChunkKind::OutputSignature,
            DxbcChunkKind::ShaderProgram,
            DxbcChunkKind::Statistics,
        ]
    );
    assert_eq!(summary.chunks[0].offset, 52);
    assert_eq!(summary.chunks[0].size, 4);
    assert_eq!(summary.chunks[0].byte_range, ByteRange { start: 52, len: 12 });
    assert_eq!(summary.chunks[3].byte_range, ByteRange { start: 96, len: 28 });
    assert_eq!(summary.chunks[4].byte_range, ByteRange { start: 124, len: 12 });

    assert_eq!(summary.shaders.len(), 1);
    let shader = summary.shaders[0];
    assert_eq!(shader.chunk_index, 3);
    assert_eq!(shader.version_token, SHEX_VERSION_TOKEN);
    assert_eq!(shader.length_token, 5);
    assert_eq!(shader.instruction_count, 2);
    assert_eq!(shader.instruction_words, 3);
}

#[test]
fn shader_stream_reports_instruction_ranges() {
    let source = memory_source(minimal_container());
    let mut ledger = BudgetTracker::new(Budget::default());

    let container = DxbcContainer::open(&source, &mut ledger).expect("open container");
    assert_eq!(container.chunk_offsets(), &[52, 64, 80, 96, 124]);
    let chunks = container.chunks(&mut ledger).expect("read chunk headers");
    let program = ShexProgram::open(&source, &mut ledger, &chunks[3]).expect("open shader chunk");
    assert_eq!(program.chunk_index(), 3);
    assert_eq!(program.version_token(), SHEX_VERSION_TOKEN);
    assert_eq!(program.stream_words(), 3);

    let instructions = program.instructions(&mut ledger).map(|item| item.expect("instruction layout")).collect::<Vec<_>>();
    assert_eq!(instructions.len(), 2);
    assert_eq!(instructions[0].opcode, OPCODE_DECLARE);
    assert_eq!(instructions[0].word_count, 2);
    assert_eq!(instructions[0].word_offset, 0);
    assert_eq!(instructions[0].byte_range, ByteRange { start: 112, len: 8 });
    assert_eq!(instructions[1].opcode, OPCODE_RETURN);
    assert_eq!(instructions[1].word_count, 1);
    assert_eq!(instructions[1].word_offset, 2);
    assert_eq!(instructions[1].byte_range, ByteRange { start: 120, len: 4 });
}

#[test]
fn unknown_chunks_are_skipped_by_length_and_keep_their_range() {
    let mut chunks = minimal_chunks();
    chunks.insert(1, chunk(*b"XDBC", vec![0xAA, 0xBB, 0xCC, 0xDD]));
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let summary = validate_dxbc_bytes(&bytes).expect("an unknown chunk does not invalidate the container");
    assert_eq!(summary.chunks.len(), 6);
    assert_eq!(summary.chunks[1].kind, DxbcChunkKind::Unknown(*b"XDBC"));
    assert_eq!(summary.chunks[1].four_cc, *b"XDBC");
    assert_eq!(summary.chunks[1].size, 4);
    assert_eq!(summary.chunks[1].byte_range, ByteRange { start: 68, len: 12 });
    assert_eq!(summary.chunks[2].byte_range, ByteRange { start: 80, len: 16 });
    assert_eq!(summary.shaders[0].chunk_index, 4);
    assert_eq!(summary.header.chunk_count, 6);
}

#[test]
fn bad_magic_keeps_the_magic_range() {
    let mut bytes = minimal_container();
    bytes[0..4].copy_from_slice(b"XBCD");

    let error = validate_dxbc_bytes(&bytes).expect_err("bad magic must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::BadMagic { magic, evidence }) => {
            assert_eq!(magic, *b"XBCD");
            assert_eq!(evidence.chunk_index, None);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn declared_length_must_match_the_supplied_bytes() {
    let mut bytes = minimal_container();
    patch_u32(&mut bytes, 24, 135);

    let error = validate_dxbc_bytes(&bytes).expect_err("a length mismatch must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::LengthMismatch { declared, actual, evidence }) => {
            assert_eq!(declared, 135);
            assert_eq!(actual, 136);
            assert_eq!(evidence.byte_range, ByteRange { start: 24, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn chunk_count_whose_table_does_not_fit_is_rejected() {
    let mut bytes = minimal_container();
    patch_u32(&mut bytes, 28, 1000);

    let error = validate_dxbc_bytes(&bytes).expect_err("an oversized offset table must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ChunkCountOutOfBounds {
            chunk_count,
            declared_length,
            table_bytes,
            evidence,
        }) => {
            assert_eq!(chunk_count, 1000);
            assert_eq!(declared_length, 136);
            assert_eq!(table_bytes, 4000);
            assert_eq!(evidence.byte_range, ByteRange { start: 28, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn chunk_offset_past_the_container_is_rejected() {
    let mut bytes = minimal_container();
    patch_u32(&mut bytes, 32 + 4, 200);

    let error = validate_dxbc_bytes(&bytes).expect_err("an offset past the container must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ChunkOffsetOutOfBounds { chunk_index, offset, declared_length, evidence }) => {
            assert_eq!(chunk_index, 1);
            assert_eq!(offset, 200);
            assert_eq!(declared_length, 136);
            assert_eq!(evidence.chunk_index, Some(1));
            assert_eq!(evidence.byte_range, ByteRange { start: 200, len: 8 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn chunk_size_past_the_container_is_rejected() {
    let mut bytes = minimal_container();
    // The `ISGN` chunk header sits at 64, so its size word is at 68.
    patch_u32(&mut bytes, 68, 200);

    let error = validate_dxbc_bytes(&bytes).expect_err("a size past the container must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ChunkSizeOutOfBounds {
            chunk_index,
            four_cc,
            size,
            offset,
            declared_length,
            evidence,
        }) => {
            assert_eq!(chunk_index, 1);
            assert_eq!(four_cc, *b"ISGN");
            assert_eq!(size, 200);
            assert_eq!(offset, 64);
            assert_eq!(declared_length, 136);
            assert_eq!(evidence.chunk_index, Some(1));
            assert_eq!(evidence.byte_range, ByteRange { start: 72, len: 200 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn overlapping_chunks_are_rejected() {
    let mut bytes = minimal_container();
    // Point the second chunk at the first chunk's offset.
    patch_u32(&mut bytes, 32 + 4, 52);

    let error = validate_dxbc_bytes(&bytes).expect_err("overlapping chunks must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ChunkOverlap { first_index, second_index, evidence }) => {
            assert_eq!(first_index, 0);
            assert_eq!(second_index, 1);
            assert_eq!(evidence.chunk_index, Some(1));
            assert_eq!(evidence.byte_range, ByteRange { start: 52, len: 12 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn shader_chunk_without_room_for_its_tokens_is_rejected() {
    let mut chunks = minimal_chunks();
    chunks[3] = chunk(*b"SHEX", vec![0x00; 4]);
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let error = validate_dxbc_bytes(&bytes).expect_err("a shader chunk too small for its tokens must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ShaderChunkTooSmall { chunk_index, chunk_size, evidence }) => {
            assert_eq!(chunk_index, 3);
            assert_eq!(chunk_size, 4);
            assert_eq!(evidence.chunk_index, Some(3));
            assert_eq!(evidence.byte_range, ByteRange { start: 104, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn emitted_fill_shader_decodes_with_the_dword_length_field() {
    let mut chunks = minimal_chunks();
    chunks[3] = chunk(*b"SHEX", EMITTED_FILL_SHEX.to_vec());
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let summary = validate_dxbc_bytes(&bytes).expect("an emitted fill shader is structurally valid");
    assert_eq!(summary.header.total_length, 324);
    assert_eq!(summary.chunks[3].size, 208);

    let shader = summary.shaders[0];
    assert_eq!(shader.version_token, 0x0005_0051);
    assert_eq!(shader.length_token, 52);
    assert_eq!(shader.instruction_count, 13);
    assert_eq!(shader.instruction_words, 50);
}

#[test]
fn emitted_fill_instruction_ranges_cover_the_whole_stream() {
    let mut chunks = minimal_chunks();
    chunks[3] = chunk(*b"SHEX", EMITTED_FILL_SHEX.to_vec());
    let source = memory_source(build_container(CONTAINER_VERSION, &chunks));
    let mut ledger = BudgetTracker::new(Budget::default());

    let container = DxbcContainer::open(&source, &mut ledger).expect("open container");
    let chunks = container.chunks(&mut ledger).expect("read chunk headers");
    let program = ShexProgram::open(&source, &mut ledger, &chunks[3]).expect("open shader chunk");
    let instructions = program.instructions(&mut ledger).map(|item| item.expect("instruction layout")).collect::<Vec<_>>();

    assert_eq!(instructions.len(), 13);
    assert_eq!(
        instructions[0],
        DxbcInstruction { opcode: 0x006A, word_count: 1, word_offset: 0, byte_range: ByteRange { start: 112, len: 4 } }
    );
    assert_eq!(instructions[1].opcode, 0x0059);
    assert_eq!(instructions[1].word_count, 7);
    assert_eq!(instructions[1].word_offset, 1);
    assert_eq!(instructions[1].byte_range, ByteRange { start: 116, len: 28 });

    let last = instructions[12];
    assert_eq!(last.opcode, 0x003E);
    assert_eq!(last.word_count, 1);
    assert_eq!(last.word_offset, 49);
    assert_eq!(last.byte_range, ByteRange { start: 308, len: 4 });

    let decoded_words: u64 = instructions.iter().map(|instruction| u64::from(instruction.word_count)).sum();
    assert_eq!(decoded_words, 50, "the walk must consume exactly the DWORDs after the token pair");
}

#[test]
fn shader_length_token_is_counted_in_dwords() {
    // A 52-DWORD shader in a 208-byte chunk: the ratio a real `fxc` product uses.
    let words = vec![instruction_token(OPCODE_RETURN, 1); 50];
    let mut chunks = minimal_chunks();
    chunks[3] = chunk(*b"SHEX", shex_payload(SHEX_VERSION_TOKEN, 52, &words));
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let summary = validate_dxbc_bytes(&bytes).expect("52 DWORDs and 208 bytes describe the same payload");
    assert_eq!(summary.chunks[3].size, 208);
    assert_eq!(summary.shaders[0].length_token, 52);
    assert_eq!(summary.shaders[0].instruction_count, 50);
    assert_eq!(summary.shaders[0].instruction_words, 50);
}

#[test]
fn shader_length_token_must_match_the_chunk_size() {
    // The payload is 20 bytes, so a token of 4 DWORDs claims only 16 bytes.
    let mut chunks = minimal_chunks();
    let words = [instruction_token(OPCODE_DECLARE, 2), 0x0000_0001, instruction_token(OPCODE_RETURN, 1)];
    chunks[3] = chunk(*b"SHEX", shex_payload(SHEX_VERSION_TOKEN, 4, &words));
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let error = validate_dxbc_bytes(&bytes).expect_err("a length token mismatch must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ShaderLengthMismatch { chunk_index, declared, chunk_size, evidence }) => {
            assert_eq!(chunk_index, 3);
            assert_eq!(declared, 4);
            assert_eq!(chunk_size, 20);
            assert_eq!(evidence.chunk_index, Some(3));
            assert_eq!(evidence.byte_range, ByteRange { start: 108, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn shader_length_mismatch_message_names_both_units() {
    let mut chunks = minimal_chunks();
    let words = [instruction_token(OPCODE_DECLARE, 2), 0x0000_0001, instruction_token(OPCODE_RETURN, 1)];
    chunks[3] = chunk(*b"SHEX", shex_payload(SHEX_VERSION_TOKEN, 4, &words));
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let message = validate_dxbc_bytes(&bytes).expect_err("a length token mismatch must fail").to_string();
    assert!(message.contains("4 DWORDs"), "{message}");
    assert!(message.contains("20-byte"), "{message}");
}

#[test]
fn zero_word_count_instruction_is_rejected() {
    let mut chunks = minimal_chunks();
    chunks[3] = chunk(*b"SHEX", shex_payload(SHEX_VERSION_TOKEN, 5, &[instruction_token(OPCODE_DECLARE, 0), 0, 0]));
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let error = validate_dxbc_bytes(&bytes).expect_err("a zero word count must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::ZeroWordCount { evidence }) => {
            assert_eq!(evidence.chunk_index, Some(3));
            assert_eq!(evidence.byte_range, ByteRange { start: 112, len: 4 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn instruction_past_the_stream_end_is_rejected() {
    let mut chunks = minimal_chunks();
    let words = [instruction_token(OPCODE_DECLARE, 9), 0, 0];
    chunks[3] = chunk(*b"SHEX", shex_payload(SHEX_VERSION_TOKEN, 5, &words));
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let error = validate_dxbc_bytes(&bytes).expect_err("a declared length past the stream must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::InstructionOutOfBounds { word_count, available_words, evidence }) => {
            assert_eq!(word_count, 9);
            assert_eq!(available_words, 3);
            assert_eq!(evidence.chunk_index, Some(3));
            assert_eq!(evidence.byte_range, ByteRange { start: 112, len: 12 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn trailing_bytes_that_do_not_complete_a_word_are_rejected() {
    let mut chunks = minimal_chunks();
    let words = [instruction_token(OPCODE_DECLARE, 2), 0, instruction_token(OPCODE_RETURN, 1)];
    // A 21-byte payload cannot be described by any DWORD count, so the alignment
    // check fires before the length token is compared.
    let mut payload = shex_payload(SHEX_VERSION_TOKEN, 5, &words);
    payload.push(0x7F);
    chunks[3] = chunk(*b"SHEX", payload);
    let bytes = build_container(CONTAINER_VERSION, &chunks);

    let error = validate_dxbc_bytes(&bytes).expect_err("a partial trailing word must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::TrailingPartialWord { remainder_bytes, evidence }) => {
            assert_eq!(remainder_bytes, 1);
            assert_eq!(evidence.chunk_index, Some(3));
            assert_eq!(evidence.byte_range, ByteRange { start: 124, len: 1 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_entry_rejects_a_truncated_header() {
    let bytes = minimal_container()[..16].to_vec();

    let error = validate_dxbc_bytes(&bytes).expect_err("a truncated header must fail");
    match error {
        DxbcByteError::Layout(DxbcLayoutError::TruncatedHeader { available_bytes, evidence }) => {
            assert_eq!(available_bytes, 16);
            assert_eq!(evidence.byte_range, ByteRange { start: 0, len: 16 });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn byte_slice_error_keeps_the_original_error() {
    let mut bytes = minimal_container();
    bytes[0..4].copy_from_slice(b"XBCD");

    let error = validate_dxbc_bytes(&bytes).expect_err("bad magic must fail");
    assert!(error.to_string().contains("magic"), "{error}");
    assert!(std::error::Error::source(&error).is_some(), "the typed cause must be preserved");
}

#[test]
fn byte_error_maps_layout_budget_and_source_failures() {
    assert_eq!(DxbcByteError::from(DxbcLayoutError::UnknownLength), DxbcByteError::Layout(DxbcLayoutError::UnknownLength));
    assert_eq!(
        DxbcByteError::from(DxbcLayoutError::Budget(BudgetError::ReadBytes)),
        DxbcByteError::Budget(BudgetError::ReadBytes)
    );
    assert_eq!(DxbcByteError::from(BudgetError::ScanBytes), DxbcByteError::Budget(BudgetError::ScanBytes));
}

#[test]
fn budget_is_checked_before_the_header_is_read() {
    let source = memory_source(minimal_container());
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: 16, ..Budget::default() });

    let error = validate_dxbc(&source, &mut ledger).expect_err("a small read budget must fail");
    assert_eq!(error, DxbcLayoutError::Budget(BudgetError::ReadBytes));
}

#[test]
fn budget_is_checked_while_chunk_headers_are_read() {
    let source = memory_source(minimal_container());
    let mut ledger = BudgetTracker::new(Budget { max_read_bytes: 52, ..Budget::default() });

    let container = DxbcContainer::open(&source, &mut ledger).expect("the header and offset table fit the allowance");
    let error = container.chunks(&mut ledger).expect_err("the first chunk header exceeds the allowance");
    assert_eq!(error, DxbcLayoutError::Budget(BudgetError::ReadBytes));
}
