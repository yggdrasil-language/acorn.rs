# acorn-tar

Tar container facts, reading, writing and tar-gzip composition for Acorn.

## 🎯 Responsibilities

- **Indexing**: `index_tar` / `index_tar_bytes` walk 512-byte headers and return a `TarArchive` with one `TarEntry` per member. Each entry records its path, link target, `TarEntryKind`, mode, uid/gid, size, Unix modification time, header offset and declared payload range.
- **Header formats**: `parse_header` validates the header checksum and decodes both octal and base-256 numbers, so archives produced by GNU tar with huge or negative values are indexed instead of rejected. Ustar name and prefix fields are joined into the member path.
- **Metadata records**: GNU `L` / `K` long names and link targets are consumed and attached to the following entry. PAX `x` / `g` records are parsed for `path`, `linkpath`, `size` and `mtime`, with `MAX_METADATA_BYTES` bounding how much a single archive can spend on metadata before the following entry.
- **Payloads**: `read_member_payload` reads one member through the source and accounts the read against the archive budget. Payload ranges are 512-byte aligned and a short or truncated block is reported instead of silently clamped.
- **Writing**: `TarWriter` / `write_tar` emit ustar headers, switch to GNU long-name and long-link records when a path or link target no longer fits, and terminate the archive with two zero blocks. `TarEntryDraft` describes files, directories, symlinks and hard links, and rejects data on a non-regular entry and a link target on a non-link entry.
- **Tar-gzip**: `TarGz` composes a tar payload with one gzip member. `TarGz::decode` inflates through `acorn-codec`'s `Gzip`, rejects trailing input, and indexes the tar bytes without copying them. `TarGz::encode` wraps a tar buffer in the same in-repo gzip encoder.
- **Failure stays structured**: `TarIndexError` separates truncation, a missing end-of-archive marker, checksum mismatch, invalid header fields, oversize metadata, metadata without an entry and `NeedRange`. `TarReadError` and `TarWriteError` keep read and write failures out of the index error space.

## 🔗 Dependencies

- Layout and budget contracts come from `acorn-core`, reads come from `acorn-source`.
- gzip for `.tar.gz` is `acorn-codec`'s `Gzip`, enabled through the `compression` feature. Tar header layout, record semantics and checksum meaning are owned here, not by the codec.
- `acorn-tar` owns tar container facts only: 512-byte headers, ustar prefix, GNU long records, PAX overrides, payload alignment and the end-of-archive marker.

## 🚧 Boundaries

- It is not a compression layer. gzip bytes come from `acorn-codec` and the crate never delegates to an external archive or compression library.
- It does not materialize an entire archive by default, and it does not decide document semantics.
- Paths are kept as raw strings and are never normalized, joined with a root or checked for traversal. Path safety, Windows reserved names, alternate data streams, archive-bomb thresholds and overwrite policy are product policy and are not part of this crate.
- It does not follow symlinks, hard links or device entries, and it never writes archive contents back to disk.
- xz, bzip2, zstd and 7z containers are out of scope. Only tar and tar-gzip are implemented here.

## 🤝 Contributing

Contributions are welcome! Keep header checksum and numeric decoding validated before a member is accepted, and keep budget checks before payload allocation.
