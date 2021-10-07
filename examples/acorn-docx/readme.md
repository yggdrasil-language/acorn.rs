# acorn-docx

DOCX OPC package layout on top of `acorn-zip`.

## 🎯 Responsibilities

- **Package access**: `OpcPackage` opens a DOCX OPC package and `normalize_part_path` produces a canonical part path. `OpcError` reports package-level failures.
- **Relationships**: `parse_relationships` / `parse_relationship_targets` read `.rels` parts, `resolve_opc_target` resolves a relationship target relative to the part that referenced it, and `OFFICE_DOCUMENT_RELATIONSHIP_TYPE` / `ROOT_RELS_PATH` identify the root relationship and its path.

## 🔗 Dependencies

- Container structure, member access and member decoding come from `acorn-zip`. This crate adds only OPC parts, relationships and package provenance.
- WordprocessingML (the document XML itself) is parsed by Oak in later slices; this crate does not own document text semantics.

## 🚧 Boundaries

- No part URI resolution is done by fetching anything external: a target outside the package is reported, not retrieved.
- Part identity is the full normalized package path, so `a/image.png` and `b/image.png` are never merged by basename.
- ZIP structure and codec logic are never duplicated here.

## 🤝 Contributing

Contributions are welcome! Keep part and relationship semantics in this crate and ZIP mechanics in `acorn-zip`.
