# 🔍 Acorn Probe

**Magic-byte probing and format candidate collection for Acorn** — `acorn-probe` turns raw bytes into ranked format hypotheses with evidence ranges, without ever claiming that a hypothesis is validated structure.

## 🎯 Responsibilities

- **`Signature`**: a registered pattern with an anchor (`ProbeAnchor::Start`, `Fixed(offset)`, `Tail { .. }` or `Search { start, len }`), a priority and a format id.
- **`BytePattern`**: the byte pattern and its mask, so a signature can match a field with fixed and variable bits.
- **`ProbeSession`**: registers signatures, sorts them by descending priority, and probes a source. It returns every candidate together with the byte ranges that produced it.
- **`ProbeBudget`**: bounds the signature scan (`max_scan_bytes`, `max_hits_per_signature`). Probing is a budgeted parse action, not an unbounded search.
- **`Confidence`**: `Hint`, `Candidate`, `Identified`. A magic hit alone is `Candidate` at most; `Identified` is reserved for a later structural validation phase.
- **`document_container_signatures()`** and **`probe_memory()`**: built-in signatures for container-first document pipelines (ZIP, gzip, zlib, PDF) and a convenience probe over in-memory bytes.

## 🏗️ Architecture

- `Signature` — what to look for, and where it is allowed to appear.
- `ProbeCandidate` — a format id, a confidence tier, the evidence ranges in source coordinates, an optional version hint and human-readable reasons.
- `ProbeSession` — the registry plus the bounded scan.

## ⚠️ Contract

A probe result states what matched and where. It does not state that the input is valid, and it never executes or extracts anything from the input. Downstream format adapters decide whether a candidate can be structurally validated, and `acorn-core`'s node states keep `absent`, `unknown`, `unsupported` and `truncated` distinct from each other.

## 🔗 Ecosystem

`acorn-probe` depends on `acorn-source` for reads and on the shared diagnostic crate for records. `acorn-core` depends on it, and `acorn` re-exports its contracts at the crate root.

## 🤝 Contributing

Contributions are welcome! Add signatures with the narrowest anchor that still matches the format, and keep the confidence tier honest.
