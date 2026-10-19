# 🌰 Acorn — Binary Format Parsing Framework for Rust

[![Rust Version](https://img.shields.io/badge/rust-edition%202024-blue.svg)](https://www.rust-lang.org)

Acorn is a modular framework for parsing **binary** formats in Rust. Where a text parser turns a source file into a
syntax tree, Acorn turns a byte source into **address spaces, a bounded layout graph, checked references and lazy field
views** — with the evidence for every claim it makes.

Acorn is the binary sibling of [Oaks](https://github.com/yggdrasil-language/oak.rs): the two share a workspace shape
(`projects/*` infrastructure + `examples/*` format adapters + one aggregate facade) but not a data model. Acorn does
**not** reuse lexers, green/red trees, line/column source maps or a universal AST.

## 🧭 One Line of Boundary

```text
ByteSource
  → magic / envelope probe
  → bounded layout index
  → lazy field decode + checked references
  → format view / validation / diagnostics
```

|                | Oaks                             | Acorn                                   |
|----------------|----------------------------------|-----------------------------------------|
| Input          | text source                      | byte source                             |
| Core model     | lexer / parser / CST / typed AST | probe / layout graph / lazy binary view |
| First artifact | lossless syntax tree             | lazy layout graph with evidence         |

## 🌟 Why Acorn?

- **Layout graph over tree.** Pointers, directories and back-references form graphs that may contain cycles. Acorn
  models ranges, fields, endianness, references and evidence directly instead of forcing a parent/child tree onto them.
- **Lazy by budget.** The index stage reads only the ranges it needs. Large payloads, compressed members and deep
  directories expand on demand, and every read, decode and cache byte is accounted for against a `Budget`.
- **Evidence first.** Every identification and decode result records its source range, rule version and validation
  state. A magic hit is never presented as fully identified structure
  (`candidate ≠ identified ≠ structurally valid ≠ semantically valid`).

## ✨ Features

- **Explicit address spaces.** File offsets, segment-relative offsets, virtual addresses, decompressed logical offsets
  and object ids are distinct typed spaces, never a shared bare integer.
- **Bounded, non-padding sources.** `ByteSource` never pads past EOF and never fabricates missing ranges. Short reads,
  unavailable intervals and missing remote ranges (`NeedRange`) are reported explicitly.
- **Format-neutral core.** `acorn-core` owns address spaces, spans, partial semantics, node states, parse budgets and
  the layout contract. It performs no I/O and no format-specific decoding.
- **Own codec implementations.** `acorn-codec` fully owns its representation transforms and its Deflate / zlib / gzip
  decoders. No external compression backend is used as a production path, and no format adapter re-implements them.
- **Format adapters stay format-local.** An adapter contributes that format's headers, directories, records and checksum
  meaning. It does not copy `ByteSource`, the address space or the budget, and it does not become a universal adapter
  behind a feature flag.
- **Failure stays visible.** Truncation, unknown values, resource limits and unsupported features are preserved in
  results instead of silently degrading into empty fields.
- **Safe by construction.** Parsing never executes the input, never loads or links discovered executables, never
  auto-fetches external paths and never writes archive contents back to disk.

## 🏗️ Architecture

```text
format adapter (ZIP / PE / Mach-O / JVM / DEX / ...)
        ↓
format-neutral layout and reference graph
        ↓
probe / envelope registry + validation
        ↓
ByteSource / range cache / bounded reader
```

- `ByteSource` reads addressable bytes and knows nothing about ZIP or PE.
- The probe layer produces ranked candidates from magic bytes, version hints, length constraints and local structure. It
  does not assert semantic validity.
- The layout layer records ranges, fields, endianness, references and evidence, and allows nodes to appear out of file
  order.
- A format adapter owns that format's headers, directories, records and checksum projection.
- The facade combines candidates and envelope layers and exposes query, export and diagnostics.

Dependency direction:

```text
acorn-codec ──→ core / alloc / std          (own Deflate, no external backend)
acorn-source ─→ diagnostic
acorn-probe ──→ acorn-source + diagnostic
acorn-core ───→ acorn-source + acorn-probe + diagnostic
format adapter → acorn-core + acorn-codec + format-local deps
acorn ────────→ acorn-core + acorn-source + acorn-codec + acorn-probe
```

## 📦 Core Components

| Component                                 | Description                                                                        | Status    |
|-------------------------------------------|------------------------------------------------------------------------------------|-----------|
| [`acorn-core`](./projects/acorn-core)     | Address spaces, spans, layout graph, references, node states, budgets, write plans | ✅ Active |
| [`acorn-source`](./projects/acorn-source) | `ByteSource`, windows, in-memory sources/sinks, range maps, source identity        | ✅ Active |
| [`acorn-codec`](./projects/acorn-codec)   | Bit I/O, integer/representation transforms, Deflate / zlib / gzip                  | ✅ Active |
| [`acorn-probe`](./projects/acorn-probe)   | Magic-byte signatures, probe candidates, container registry                        | ✅ Active |
| [`acorn-host`](./projects/acorn-host)     | Minimal per-platform host executable wrappers                                      | ✅ Active |
| [`acorn`](./projects/acorn)               | Format-neutral aggregate facade                                                    | ✅ Active |

`acorn` re-exports the core contracts directly at the crate root and does **not** register or re-export any format
adapter in `examples/`. Consumers depend on `acorn` plus the specific format crate they need. Compression is not a
default dependency: `acorn-codec` and `acorn` expose non-compression transforms by default, and a real format adapter
opts into the `compression` feature explicitly.

## 🧩 Format Adapters

`examples/*` holds the format crates and end-to-end samples. They are independent: each owns its own semantics and is
added to the workspace only once it has real samples and structural tests.

| Adapter                                     | Description                                          |
|---------------------------------------------|------------------------------------------------------|
| [`acorn-zip`](./examples/acorn-zip)         | ZIP central-directory indexer and member decoding    |
| [`acorn-cfb`](./examples/acorn-cfb)         | Compound File Binary Format (OLE/CFB) reader         |
| [`acorn-doc`](./examples/acorn-doc)         | Legacy Word `.doc` (OLE + Word binary) semantics     |
| [`acorn-docx`](./examples/acorn-docx)       | DOCX OPC package layout on top of `acorn-zip`        |
| [`acorn-epub`](./examples/acorn-epub)       | EPUB OCF container and OPF manifest/spine indexing   |
| [`acorn-pe`](./examples/acorn-pe)           | PE / COFF / ELF binary format                        |
| [`acorn-mach-o`](./examples/acorn-mach-o)   | Apple Mach-O binary format                           |
| [`acorn-wasm`](./examples/acorn-wasm)       | WebAssembly binary module model                      |
| [`acorn-jvm`](./examples/acorn-jvm)         | JVM `class` and `JAR` binary format                  |
| [`acorn-dex`](./examples/acorn-dex)         | Android DEX (`dex/035`) binary format                |
| [`acorn-tvm`](./examples/acorn-tvm)         | TextVM `.tvm` regex engine artifact format           |
| [`acorn-x64`](./examples/acorn-x64)         | x86-64 instruction encoding and native PE/ELF images |
| [`acorn-aarch64`](./examples/acorn-aarch64) | AArch64 instruction encoding and Android JNI glue    |

## 🛠️ Quick Start

Probe container magic bytes and get ranked candidates with evidence ranges:

```rust
use acorn::{MemorySource, ProbeSession, probe};

fn main() {
    let bytes = std::fs::read("archive.zip").expect("read input");
    let source = MemorySource::from_bytes("archive.zip", bytes);

    let mut session = ProbeSession::new();
    for signature in probe::document_container_signatures() {
        session.register(signature);
    }

    for candidate in session.probe(&source) {
        println!("{} {:?} {:?}", candidate.format_id, candidate.confidence, candidate.evidence_ranges);
    }
}
```

For container work, add the format adapter explicitly:

```toml
[dependencies]
acorn = { path = "projects/acorn" }
acorn-zip = { path = "examples/acorn-zip" }
```

An adapter that needs decompression enables `acorn-codec/compression` in its own manifest, so ordinary binary parsers do
not link a Deflate implementation at all.

## 📂 Project Structure

```text
acorn.rs/
├── Cargo.toml            # Workspace configuration
├── projects/             # Format-neutral infrastructure
│   ├── acorn-core/       # Address spaces, layout graph, budgets
│   ├── acorn-source/     # ByteSource, windows, sinks
│   ├── acorn-codec/      # Representation transforms and compression
│   ├── acorn-probe/      # Signatures and candidates
│   ├── acorn-host/       # Per-platform host executables
│   └── acorn/            # Aggregate facade
├── examples/             # Format adapters and end-to-end samples
│   ├── acorn-zip/
│   ├── acorn-docx/
│   └── ...               # see the adapter table above
└── ...
```

## 🔨 Build & Development

- Rust toolchain with Cargo workspace support (edition 2024).
- `cargo test --workspace` must pass.
- `cargo clippy --workspace --all-targets --all-features -D warnings` must pass on supported toolchains.
- Compression code paths are gated behind the `compression` feature. Consumers that only need integer or representation
  transforms should not link any Deflate implementation.

## 📊 Development Status

### ✅ Landed

- **Core contracts**: address spaces, spans, `LayoutGraph`, references with evidence, node states and `Budget`.
- **Sources**: `MemorySource`, `ByteWindow`, `RangeMapSource` with `provide_range` for late-arriving ranges, and source
  identity that carries a content digest so a cached index can be proven current or stale.
- **Budget ledger**: `BudgetTracker` separates reserve, settle and release, accounts reads and decodes separately,
  tracks peak retained bytes and rejects unreserved settles.
- **Codec**: typed transform families (`ZigZag`, `Leb128`, `Base64`, `Hex`) and a `compression` layer holding exactly
  `Deflate`, `Zlib` and `Gzip` with own decoders and RFC-based tests.
- **Block emit primitives**: `Symbol`, `StoredBlock`, `FixedBlock`, `DeflateBlock`, `DeflateBlockPlan` and `write_block`
  for orchestrators that must control block sequence, `BFINAL` placement and bit alignment block by block.
- **ZIP members**: declared payload range and declared sizes are checked before allocating, and member failures keep the
  codec category, byte offset and bit position instead of collapsing into a single string.
- **Missing-range paths**: `ZipIndexError` and `ZipViewError` can report `NeedRange` rather than degrading into `NotZip`
  or an invalid member.

### 🔄 In Progress

- **Incremental Deflate**: the state machine and bounded buffer exist, but `StreamDecoder` is still a bounded-buffering
  wrapper. A resumable bit / Huffman / window state machine is not closed yet.
- **Shared budget scope**: the ledger exists, but format layers still compare raw `Budget` limits directly instead of
  settling one shared document-scoped allowance through `BudgetTracker`.
- **ZIP indexing**: indexes are still materialized through a full read; on-demand indexing bound to one source snapshot
  is not complete.
- **PDF / PNG ownership**: Flate paths in downstream consumers still use an external compression library and are being
  migrated to Acorn `Zlib` / `Gzip`.

### 📋 Planned

- **Write path**: `ByteSink`, layout plans, relocations, patching and post-write verification, with every writer
  required to prove that its output can be re-read.
- **Format ownership migration**: PDF object/xref/page readers and writers and PNG chunk/filter handling move into
  format crates with real samples.
- **Diagnostics end to end**: the same error keeps the same code, source identity, range and budget reason across Rust,
  JSON diagnostics and downstream adapters.

## 🤝 Contributing

Contributions are welcome! Please open an issue before large changes.

When adding a format adapter:

1. Create a directory under `examples/` named `acorn-{format}`.
2. Own that format's headers, directories, records and checksum meaning; reuse `acorn-core`, `acorn-source` and
   `acorn-codec` instead of copying them.
3. Accept real samples and structural tests before adding the crate to the workspace.
4. Cover normal, truncated, out-of-range, wrong-checksum, unknown-version and budget-exhaustion inputs.

Adapters must never add format-specific branches to the core to make a sample pass.

## 📚 Resources

- [Repository](https://github.com/yggdrasil-language/acorn.rs)
- [Issue Tracker](https://github.com/yggdrasil-language/acorn.rs/issues)
- Development documentation in each crate's `readme.md`
- Samples and adapters in the `examples/` directory

---

**Parse bytes, keep the evidence.** 🌰
