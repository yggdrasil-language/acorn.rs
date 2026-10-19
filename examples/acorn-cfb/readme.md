# acorn-cfb

Acorn-owned reader for Compound File Binary Format (OLE/CFB).

## 🎯 Responsibilities

- **Header and FAT**: `COMPOUND_FILE_SIGNATURE` and `is_compound_file` identify the container, and the header, FAT and directory chain are walked inside this crate.
- **Directory access**: `CompoundFile` exposes the storage/stream tree and stream bytes.
- **Errors**: `CfbError` reports structural failures structurally rather than as strings.

## 🔗 Dependencies

- Reads go through `acorn-source`; layout and range types come from `acorn-core`.

## 🚧 Boundaries

- Production parsers consume this crate. A third-party `cfb` crate is allowed only in dev oracle tests, never as a production dependency.
- It owns CFB container structure only. Legacy Word semantics on top of CFB belong to `acorn-doc`.

## 🤝 Contributing

Contributions are welcome! Keep oracle-only dependencies in dev scope.
