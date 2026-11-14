# acorn-dxbc

`DXBC` container and `SHEX` instruction-stream layout for Acorn.

## 🎯 Responsibilities

- **Container header**: the `DXBC` magic, the 16-byte digest, the version word, the declared total length, the chunk
  count and the chunk offset table — all decoded as recorded bytes.
- **Chunks**: every chunk is a FourCC plus a declared payload size. `RDEF`, `ISGN`, `OSGN`, `SHEX`/`SHDR` and `STAT` are
  classified; any other FourCC is kept verbatim, skipped by its declared length and still records the interval it
  occupies.
- **`SHEX` instruction layout**: a shader chunk starts with a version token and a length token, followed by a stream of
  `word_count << 16 | opcode` words. `ShexProgram::instructions` walks that stream one instruction at a time and reports
  the raw `opcode`, the declared `word_count`, the instruction's word offset and its byte range inside the container.
- **Structural validation**: `validate_dxbc` and `DxbcContainer::validate` return a `DxbcSummary` for a structurally
  valid container, or a classified `DxbcLayoutError` that keeps the chunk index and byte range of the failure.
- **Byte-slice entry**: `validate_dxbc_bytes(&[u8])` wraps the slice in an in-memory source and a default ledger, so a
  caller that only emits or loads bytes never names an Acorn source or budget type. Failures stay typed as
  `DxbcByteError`.

## 🚧 Boundary

- Layout and structural checks only. Opcodes, operand meaning, resource bindings, signatures and statistics are never
  interpreted, and no opcode table is embedded.
- The digest is recorded, never recomputed: this crate does not claim to verify signatures or content hashes.
- The write side is out of scope: this crate reads and checks emitted bytes but never emits them.
- No third-party DXBC or Direct3D toolchain is used, wrapped or re-exported, so no `d3dcompiler` is required.

## ⚠️ Errors

`DxbcLayoutError` keeps failures distinct instead of collapsing them into one string: a bad magic, a declared length that
differs from the supplied bytes, a chunk count whose offset table does not fit, a chunk offset or chunk size that leaves
the container, overlapping chunks, a shader chunk too small for its token pair, a shader length token that disagrees
with its chunk size, an instruction with a zero word count, an instruction whose word count runs past the stream,
trailing bytes that do not complete a 32-bit word, short or unavailable reads and budget exhaustion.

Every layout failure carries `DxbcEvidence`, the byte range it describes together with the chunk index it belongs to.

`DxbcByteError` keeps the typed `DxbcLayoutError` it came from, and gives budget exhaustion and source read failures
their own variants instead of a message string.

## 🔗 Ecosystem

Reads go through `acorn_source::ByteSource` and every read is settled against the caller's
`acorn_core::BudgetTracker`, so a container is loaded under the same allowance as the rest of a parse session.
`validate_dxbc_bytes` supplies both for callers without Acorn types at hand.

## 🤝 Contributing

Contributions are welcome! Keep this crate structural: a chunk is a FourCC and a range, an instruction is a length and a
range, never a decoded command.
