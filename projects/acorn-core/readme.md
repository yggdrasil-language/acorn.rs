# 💎 Acorn Core

**The format-neutral contract at the center of Acorn** — `acorn-core` freezes how binary structure is addressed, referenced and budgeted, so that every format adapter speaks the same language about layout.

This crate performs no I/O and no format-specific decoding. It does not know what a ZIP file or a PE image is.

## 🎯 Responsibilities

- **Address spaces**: file offset, segment-relative offset, virtual address, decompressed logical offset and object id are distinct spaces. `Address` and `AddressSpaceKind` keep them apart so a bare integer can never drift between meanings.
- **Spans and provenance**: `Span`, `Provenance` and `ProvenanceStep` record which source range and which rule produced a fact.
- **Layout graph**: `LayoutGraph`, `LayoutNode`, `LayoutNodeId`, `ReferenceEdge` and `evidence_absolute` express ranges, fields and references as a graph. Nodes may appear out of file order and the graph may contain cycles, so it is deliberately not a tree.
- **Partial semantics**: `Availability` and `NeedRange` let an index say "this range was not available" and be resumed later instead of padding or pretending EOF.
- **Node state**: `NodeState` distinguishes absent, unknown, unsupported and truncated from each other and from a decoded value.
- **Checked references**: `ReferenceKind`, `ReferenceEvidence`, `VisitKey`, `checked_absolute_jump` and `checked_relative_jump` validate a jump against its space and bounds and report `JumpError` when it does not hold.
- **Budgets**: `Budget`, `BudgetError` and `BudgetTracker` cover cumulative reads, current output, peak cache, steps, members, envelope depth and cancellation. A reservation occupies allowance until settled, so a failed or cancelled path still accounts for what it consumed.
- **Write plans**: `LayoutPlan`, `PlanSlot`, `PlanSlotId`, `PatchPoint`, `Relocation`, `RelocationId`, `RelocationTarget`, `Endian` and `PlanError` describe planned output separately from the read-side `LayoutGraph`.

## 🏗️ Architecture

- `Address` / `ByteRange` — typed coordinates and ranges in a declared space.
- `LayoutGraph` — the format-neutral result of an index pass.
- `Reference` — a checked edge between nodes with retained evidence.
- `Budget` / `BudgetTracker` — the allowance and the ledger that consumes it.

## 🔗 Ecosystem

Every format adapter builds on this crate. `acorn-probe` and `acorn-source` are the other two core contracts, and `acorn` re-exports all of them at its crate root. A writer path shares `ByteSink` from `acorn-source` but keeps its plan separate from the read graph.

## 🤝 Contributing

Contributions are welcome! Keep format semantics out of this crate.
