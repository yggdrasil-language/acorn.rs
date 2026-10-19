# acorn-epub

EPUB Open Container Format (OCF) and OPF package indexing for Acorn.

## 🎯 Responsibilities

- **Container discovery**: `OcfPackage` opens an EPUB container and `ContainerRootFile` reports the root file declared by `META-INF/container.xml`. `OcfError` reports container-level failures.
- **OPF indexing**: `parse_opf_xml` produces an `OpfDocument` with `OpfMetadata`, `ManifestItem` entries and `SpineItem` ordering.
- **Member paths**: `normalize_member_path` canonicalizes package member paths.
- **XML helpers**: `parse_xml_bytes`, `document_root`, `element_is`, `elements_by_local_name` and `attribute_value` support the indexing pass, with XML tokenization delegated to Oak.

## 🔗 Dependencies

- Container structure and member access come from `acorn-zip`.
- This crate owns container discovery and manifest/spine metadata only.

## 🚧 Boundaries

- It does not own XHTML or CSS semantics, rendering or reading order beyond the default spine.
- ZIP mechanics and codec logic are never duplicated here.

## 🤝 Contributing

Contributions are welcome! Keep container and package metadata here and content semantics downstream.
