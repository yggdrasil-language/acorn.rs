# acorn-doc

Acorn-owned reader for legacy Word `.doc` (OLE + Word binary) semantics.

## 🎯 Responsibilities

- **Document open**: `open_legacy_doc` and `LegacyDoc` open a `.doc` container. `DocError` reports structural failures.
- **Text extraction**: `extract_text_from_bytes` and `ExtractedText` produce text from the FIB / CLX / piece-table layout, with `fib_clx_location`, `decode_piece_table` and `decode_text_range` exposing the intermediate steps.

## 🔗 Dependencies

- Container structure comes from `acorn-cfb`.
- `notedown-formats` and Panduck consume this crate for FIB, CLX and piece-table text extraction.

## 🚧 Boundaries

- It owns the legacy Word binary semantics only: the container is never re-parsed here, and DOCX/OPC is a different crate.
- It produces extracted text and its provenance, not a document IR. Mapping into a document model belongs downstream.

## 🤝 Contributing

Contributions are welcome! Keep piece-table and text-range logic testable against real samples.
