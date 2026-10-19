# 🌰 Acorn

**The format-neutral facade of the Acorn binary parsing stack** — `acorn` re-exports the core contracts you write against, without pulling in any format adapter.

Most consumers should depend on `acorn` for the format-neutral interfaces and add the specific format crate they need (for example `acorn-zip`) separately. This facade does **not** register or re-export the adapters in `examples/`.

## 🎯 What It Re-exports

At the crate root:

- **Core layout contracts** from `acorn-core`: `Address`, `AddressSpaceId`, `AddressSpaceKind`, `Budget`, `ByteRange`, `LayoutGraph`, `LayoutNode`, `LayoutNodeId`, `NodeState`, `Provenance`, `Span`.
- **Sources, windows and sinks** from `acorn-source`: `ByteSink`, `ByteSource`, `ByteWindow`, `MemorySink`, `MemorySource`, `ReadOutcome`, `WriteError`.
- **Probing** from `acorn-probe`: `Confidence`, `ProbeAnchor`, `ProbeBudget`, `ProbeCandidate`, `ProbeSession`, `Signature`.
- **Codec entry types** from `acorn-codec`: `CodecError`, `CodecLimits`.

Submodule access is also available, and each re-exports its crate in full:

```rust
use acorn::codec;   // re-exports acorn_codec
use acorn::probe;   // re-exports acorn_probe
use acorn::source;  // re-exports acorn_source
```

## 🔌 Features

```text
acorn
  default = []                              # non-compression transforms only
  compression = ["acorn-codec/compression"] # opt in to deflate / zlib / gzip
```

Compression is deliberately not a default dependency. A consumer that only needs integer or representation transforms should not link a Deflate implementation. A real format adapter opts in explicitly.

## 🚫 What It Deliberately Does Not Do

- It does not register, aggregate or re-export any format adapter.
- It does not create an `acorn::core` submodule; core contracts are re-exported directly at the crate root.
- It does not depend on `compression` by default.
- It does not perform I/O, probing or decoding on its own.

## 🏗️ Place in the Stack

```text
acorn  →  acorn-core + acorn-source + acorn-codec + acorn-probe
```

The facade is format-neutral and thin. Format ownership stays with `examples/*` crates, each of which owns its own headers, directories, records and checksum semantics.

## 🤝 Contributing

Contributions are welcome! Keep this crate free of format-specific branches and of format adapter registration.
