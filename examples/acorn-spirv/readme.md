# acorn-spirv

`SPIR-V` binary module layout and structural validation for Acorn.

## 🎯 Responsibilities

- **Module header**: the five little-endian `u32` header words — magic `0x07230203`, version, generator, id `bound`
  and the reserved `schema` word — decoded as recorded bytes.
- **Bounded instruction layout**: `SpirvModule::instructions` walks the stream one instruction at a time and reports the
  raw `opcode`, the declared `word_count`, the instruction's word offset and its byte range. Only the leading word of
  each instruction is read, so instruction bodies stay unread until a consumer asks for them.
- **Structural validation**: `validate_spirv` and `SpirvModule::validate` return a `SpirvSummary` for a structurally
  valid module, or a classified `SpirvLayoutError` that keeps the starting word offset and byte range of the failure.

## 🚧 Boundary

- Layout and structural checks only. Opcode names, operand meaning, capabilities, entry points and id semantics are
  never interpreted, and no opcode table is embedded.
- The write side is out of scope: this crate reads and checks emitted bytes but never emits them.
- No third-party SPIR-V library is used, wrapped or re-exported.

## ⚠️ Errors

`SpirvLayoutError` keeps failures distinct instead of collapsing them into one string: bad magic, non-zero schema,
truncated header, a zero instruction word count, an instruction whose word count runs past the stream, trailing bytes
that do not complete a 32-bit word, an empty instruction stream, short or unavailable reads and budget exhaustion.

Every layout failure carries `SpirvEvidence`, a starting 32-bit word offset together with the byte range it describes.
A declared `bound` of zero, or one above the universal `0x3FFFFF` id bound, is reported as `SpirvBoundSuspicion` and
recorded as read: it is never silently accepted as valid and never panics.

## 🔗 Ecosystem

Reads go through `acorn_source::ByteSource` and every read is settled against the caller's
`acorn_core::BudgetTracker`, so a module is loaded under the same allowance as the rest of a parse session.

## 🤝 Contributing

Contributions are welcome! Keep this crate structural: an instruction is a length and a range, not a decoded command.
