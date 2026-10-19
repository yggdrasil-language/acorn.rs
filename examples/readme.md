# 🧩 Acorn Format Adapters

A collection of binary format crates and end-to-end samples built on the Acorn core contracts. Each adapter owns its own format's headers, directories, records and checksum meaning, and reads bytes through `acorn-source` instead of copying a byte reader of its own.

## 📚 Available Adapters

### Containers & Documents

| Adapter | Description |
|---|---|
| [acorn-zip](./acorn-zip) | ZIP central-directory indexer and member decoding |
| [acorn-cfb](./acorn-cfb) | Compound File Binary Format (OLE/CFB) reader |
| [acorn-doc](./acorn-doc) | Legacy Word `.doc` (OLE + Word binary) semantics |
| [acorn-docx](./acorn-docx) | DOCX OPC package layout on top of `acorn-zip` |
| [acorn-epub](./acorn-epub) | EPUB OCF container and OPF manifest/spine indexing |

### Executable & VM Formats

| Adapter | Description |
|---|---|
| [acorn-pe](./acorn-pe) | PE / COFF / ELF binary format |
| [acorn-mach-o](./acorn-mach-o) | Apple Mach-O binary format |
| [acorn-wasm](./acorn-wasm) | WebAssembly binary module model |
| [acorn-jvm](./acorn-jvm) | JVM `class` and `JAR` binary format |
| [acorn-dex](./acorn-dex) | Android DEX (`dex/035`) binary format |
| [acorn-tvm](./acorn-tvm) | TextVM `.tvm` regex engine artifact format |

### Machine Code

| Adapter | Description |
|---|---|
| [acorn-x64](./acorn-x64) | x86-64 instruction encoding and native PE/ELF executable images |
| [acorn-aarch64](./acorn-aarch64) | AArch64 instruction encoding and Android JNI glue |

## 🏗️ Common Architecture

All adapters follow the same contract:

- **Format-local semantics** — the adapter owns headers, directories, records and checksum projection for its format.
- **Shared infrastructure** — `ByteSource`, address spaces, the layout graph and budgets come from the core crates and are never copied.
- **Bounded and lazy** — declared sizes and ranges are checked before allocation, and payloads are read on demand.
- **Explicit failure** — truncation, unknown values, unsupported features and missing ranges stay in the result rather than degrading into empty bytes.
- **Owned compression** — a format that needs Deflate opts into `acorn-codec`'s `compression` feature and calls the protocol entry it needs. No adapter brings in an external compression backend.

## 🚀 Adding a New Adapter

1. Create a directory under `examples/` named `acorn-{format}`.
2. Reuse `acorn-core`, `acorn-source` and `acorn-codec` instead of re-implementing them.
3. Add the crate to the workspace only once it has real samples and structural tests.
4. Cover normal, truncated, out-of-range, wrong-checksum, unknown-version and budget-exhaustion inputs.

See [acorn-zip](./acorn-zip) and [acorn-pe](./acorn-pe) for reference implementations.

## 🤝 Contributing

Contributions are welcome! A format adapter must never add a format-specific branch to the core to make a sample pass.
