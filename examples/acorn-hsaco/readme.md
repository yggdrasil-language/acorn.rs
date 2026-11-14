# acorn-hsaco

`HSACO` ELF code object layout and structural validation for Acorn.

## 🎯 Responsibilities

- **ELF64 header**: the identification bytes (`0x7f ELF` magic, 64-bit class, little-endian data encoding, version) and the
  fixed header fields — `e_type`, `e_machine`, `e_version`, `e_entry`, both header table offsets and counts, `e_flags` and
  `e_shstrndx` — decoded as recorded bytes.
- **Header tables**: the program header table and the section header table are bounded by the object length before any
  entry is read. Each table's declared entry count and entry size are kept, and a table that would run past the object is
  reported as an out-of-bounds table or as one partial trailing entry.
- **Segments and sections**: every program header records its type, flags and file interval, and every section header
  records its name, its class and the interval it occupies. `HsacoObject::validate` reads each entry through the caller's
  budget and returns the classified failure, with the starting byte offset and byte range, when one does not hold.
- **Key sections by name**: `.text`, `.rodata`, `.symtab`, `.strtab`/`.shstrtab`, `.note` and `.amdgpu_metadata` are
  classified from the section name table. `.amdgpu_metadata` is kept as an interval and a length only.
- **Validation summary**: `HsacoSummary` reports the header, the segments, the sections, the `.text` interval with its byte
  count, the section count, the measured symbol count and the metadata interval.
- **Byte-slice entry**: `validate_hsaco_bytes(&[u8])` wraps the slice in an in-memory source and a default ledger, so a
  caller that only emits or loads bytes never names an Acorn source or budget type. Failures stay typed as
  `HsacoByteError`.

## 🚧 Boundary

- Layout and structural checks only. AMDGPU instruction encoding, wavefront semantics, relocations and loader behaviour are
  never interpreted, and no instruction table is embedded.
- `.amdgpu_metadata` is never parsed as YAML and `.note` records are never decoded: both stay names and intervals.
- Symbol entries are counted from the declared `.symtab` size and entry size. Names, bindings and section indices are not
  resolved.
- Section names are resolved only through `e_shstrndx`. Extended numbering (`e_shnum == 0` with the real count in section 0,
  or `e_phnum == 0xFFFF`) is not followed, so an object that uses it is reported as having no `.text` section.
- The write side is out of scope: this crate reads and checks emitted bytes but never emits them.
- No third-party ELF, HIP or AMDGPU library is used, wrapped or re-exported.

## ⚠️ Errors

`HsacoLayoutError` keeps failures distinct instead of collapsing them into one string: bad magic, a class that is not
64-bit, a data encoding that is not little endian, a machine that is not `AMDGPU` (224), a header table offset, entry size
or entry count that does not fit the object, a partial trailing table entry, a section name table index that addresses no
section, a section or segment payload past the end of the object, two sections that overlap, a missing or empty `.text`
section, short or unavailable reads and budget exhaustion.

Every layout failure carries `HsacoEvidence`, a byte range relative to the object plus the section index when the claim is
inside one. `HsacoByteError` keeps the typed `HsacoLayoutError` it came from, and gives budget exhaustion and source read
failures their own variants instead of a message string.

## 🔗 Ecosystem

Reads go through `acorn_source::ByteSource` and every read is settled against the caller's `acorn_core::BudgetTracker`, so
an object is loaded under the same allowance as the rest of a parse session. `validate_hsaco_bytes` supplies both for
callers without Acorn types at hand.

## 🤝 Contributing

Contributions are welcome! Keep this crate structural: a section is a name and a range, not a decoded blob.
