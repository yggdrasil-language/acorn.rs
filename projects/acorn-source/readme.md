# 📥 Acorn Source

**Random-access byte sources for Acorn** — `acorn-source` defines the only interface through which binary formats read bytes, plus the windows, caches and sinks built on top of it.

## 🎯 Responsibilities

- **`ByteSource`**: reads bytes at an absolute offset. It never pads past EOF and never fabricates a missing range. `ReadOutcome` distinguishes `Complete`, `Partial`, `Eof` and `Unavailable` instead of quietly returning short data.
- **`ByteWindow`**: a view over an outer source interval. Its identity is derived from that interval, so two windows over different ranges are never mistaken for the same input.
- **`MemorySource`**: an in-memory source that materializes its bytes at construction and carries their content digest, so a cached index can later be proven current or stale.
- **`RangeMapSource`**: a source assembled from known ranges. `provide_range` fills a gap that was previously reported as missing, rejects byte conflicts with `ReadError::ConflictingRange`, and refuses to extend past a known total length.
- **Sinks**: `ByteSink`, `MemorySink`, `WriteOutcome` and `WriteError` are the write side, kept separate from the read path.
- **Capabilities and identity**: `SourceCapabilities`, `SinkCapabilities` and `SourceIdentity` let a consumer ask what a source can do and whether it still describes the same input.
- **Text views**: decoded text and its source mapping belong here or in a format adapter, never in the codec crate.

## 🏗️ Architecture

- `SourceCapabilities` — what a source supports (windowed reads, remote ranges, known length, and so on).
- `SourceIdentity` — label plus content digest; the digest is what lets an index decide whether it can still be reused.
- `ByteWindow` — interval-scoped view for member and segment reads.

## 🔗 Ecosystem

`acorn-probe` and `acorn-core` depend on this crate. `acorn-codec` deliberately does **not**: it is a pure representation and compression library and takes caller-provided slices. Format adapters read members and ranges through `ByteSource` and map missing ranges into `acorn-core`'s partial semantics.

## 🤝 Contributing

Contributions are welcome! Short reads and missing ranges must stay visible; do not add padding or silent EOF behavior.
