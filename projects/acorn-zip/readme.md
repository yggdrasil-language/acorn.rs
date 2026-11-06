# acorn-zip

ZIP archive layout indexer for Acorn.

## 🎯 Responsibilities

- **Indexing**: `index_zip` / `index_zip_bytes` build a `ZipArchiveIndex` from central-directory records instead of scanning members end to end. `find_eocd` / `scan_eocd_fields` locate the end-of-central-directory record and `parse_zip64_extra` handles ZIP64 fields on members.
- **Format facts**: `ZipMember` keeps the raw name bytes, `ZipNameEncoding`, `ZipEncryption`, DOS modification time, extended-timestamp Unix seconds, internal and external attributes, and directory markers. Name decoding and any UTF-8 / GBK / CP437 fallback order stay with the caller.
- **Members**: `member_payload_range` reports the byte range a member would occupy.
- **Decoding**: `decode_member` / `decode_member_with_facts` / `read_member_payload` read a member on demand and return `MemberDecodeFacts`. Stored members are handled directly through `open_stored_member` / `read_stored_payload` / `read_raw_payload`.
- **Budget first**: a member's declared payload range and both declared sizes are checked before any allocation. A read must return exactly the declared compressed size and a decode must produce exactly the declared uncompressed size, including the declared-zero case. A zero-length member explicitly requires a zero decoded length.
- **Failure stays structured**: `ZipViewError` carries `ZipIndexError` for index problems and `ZipViewError::Codec(CodecError)` for decode problems, so a member failure keeps the codec category, byte offset and bit position instead of collapsing into one string. An archive whose bytes have not been fetched yet reports `NeedRange` rather than being folded into `NotZip`.

## 🔗 Dependencies

- Layout and range contracts come from `acorn-core`, reads come from `acorn-source`.
- Compression for method 8 is `acorn-codec`'s `Deflate`, enabled through the `compression` feature. ZIP declared sizes and the declared CRC are ZIP semantics and are owned here, not by the codec.
- `acorn-zip` owns ZIP container facts only: local file header, central directory, EOCD, ZIP64, member ranges and declared checksum semantics.

## 🚧 Boundaries

- It is not a compression layer. It does not implement Deflate, and it never delegates to an external archive or compression library.
- It does not materialize an entire archive by default, and it does not decide document semantics.
- Name decoding is byte-level on purpose. The crate reports the raw bytes and the UTF-8 flag, and never chooses a legacy encoding fallback for the caller.
- Encryption is classified from general-purpose flags only. The crate does not decrypt, does not implement cryptography, and does not add a crypto dependency.
- Path safety, Windows reserved names, alternate data streams, archive-bomb thresholds and overwrite policy are product policy and are not part of this crate.
- DOCX OPC and EPUB OCF build on this crate; they add parts, relationships, manifests and provenance without re-implementing ZIP.

## 🤝 Contributing

Contributions are welcome! Keep budget checks before allocation and keep member failures mapped to stable codec categories.
