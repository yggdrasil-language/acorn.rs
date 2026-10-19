# 🔢 Acorn Codec

**Stateless representation transforms and owned stream decoders for Acorn** — `acorn-codec` owns the algorithms that turn byte representations into typed values and that decompress bounded streams. It owns no address space, no layout graph, no format field model and no terminal rendering.

This crate does not depend on `acorn-core`, `acorn-source` or any format adapter. Callers map `CodecError` and decode facts into their own budgets, provenance and format diagnostics.

## 🎯 Transform Families

Acorn separates three semantically different kinds of transform instead of folding them into one universal `encode`/`decode` trait:

| Family | Semantics | Examples |
|---|---|---|
| Scalar transform | integer-domain bijection, no bytes produced | `ZigZag` |
| Slice codec | one-shot slice transform | `Base64`, `Hex`, `Leb128` |
| Stream codec | stateful, budgeted, complete-consumption streams | `Deflate`, `Zlib`, `Gzip` |

All three share `CodecLimits`, `DecodeFacts` and `CodecError`, but they do not share methods that do not apply to them.

### Scalar

```rust
ZigZag::<i32>::encode(value) -> u32
ZigZag::<i32>::decode(value) -> i32
```

ZigZag and LEB128 are two consecutive transforms. They are never merged into a single "signed LEB128" function.

### Slice

Base64, hex and LEB128 use profiled typed entry points:

```rust
Base64::standard().with_padding(Padding::Required).encode(bytes)
Base64::url_safe_no_padding().decode(text)
Hex::strict().decode(text)
Leb128::<u32>::decode(bytes)
Leb128::<i64>::wasm().decode(bytes)
```

Results are `DecodeFacts<T>` with `value`, `input_consumed`, `output_produced`, `complete`, `trailing_input` and `canonical`. `complete` and `trailing_input` always exist together; a lone `fully_consumed` boolean is never enough.

### Compression

`compression` holds exactly three entries, and they are **not** on the same layer:

| Entry | Owns | Does not own |
|---|---|---|
| `Deflate` | RFC 1951 bitstream, block headers, Huffman symbols, literal/length-distance pairs, the 32 KiB window rule | file header, checksum, member or container concepts. Raw Deflate is this type itself; there is no second "raw envelope" entry |
| `Zlib` | RFC 1950 `CMF`/`FLG` header and its check, `CINFO` window declaration, `FDICT` bit, Adler-32 trailer, trailing-input rules | the Deflate algorithm (it calls `Deflate`), ZIP semantics |
| `Gzip` | RFC 1952 10-byte member header and flags (`FEXTRA`/`FNAME`/`FCOMMENT`/`FHCRC`), CRC32 and `ISIZE` trailer, member boundary and trailing-input rules | the Deflate algorithm (it calls `Deflate`), ZIP semantics |

Envelopes parse only their own header, trailer and checksum. The algorithm is always delegated to `Deflate`. There is no `Compression::decode` that auto-detects an envelope, and there is no `acorn-zlib` or `acorn-gzip` crate — those are protocols, not formats. ZIP is a container adapter and lives in `acorn-zip`, a different layer entirely.

One-shot and streaming entry points are two shapes of the same API:

```rust
Deflate::decode(input, limits)
Zlib::decode(input, limits)
Gzip::decode(input, limits)

let mut decoder = Gzip::decoder(limits);
decoder.push(chunk);
decoder.finish(&mut output)?;
let facts = decoder.facts();
```

`finish()` must trigger trailer, checksum and completeness validation. Reading the payload is not gzip or zlib success, and `StreamFacts.checksum` carries the verified checksum rather than a permanent `None` placeholder.

## 🧱 Block Emit Primitives

`Deflate::encode` is a whole-input compressor: it chooses the block layout and match strategy itself. A second class of consumer — a fixed-point orchestrator such as a recursive template builder — must control the block sequence, `BFINAL` placement and bit alignment block by block. Those consumers use the block primitives and never implement Huffman or LZ77 of their own.

```text
Symbol
  Literal(u8)
  Match { length: u16, distance: u16 }        # length 3..=258, distance 1..=32768

StoredBlock { data }                          # BTYPE=00
FixedBlock  { symbols }                       # BTYPE=01, symbols plus automatic EOB

DeflateBlock = Stored(StoredBlock) | Fixed(FixedBlock)

write_block(writer: &mut BitWriter, block: &DeflateBlock, final_block: bool) -> Result<(), CodecError>
write_stored_block(writer, data, final_block)
write_fixed_block(writer, symbols, final_block)

DeflateBlockPlan                              # ordered (block, final_block) list
  encode() -> Result<Vec<u8>, CodecError>     # blocks written bit-continuously, only stored blocks align internally
```

Semantics:

1. `write_block` writes only the block header, payload and end-of-block symbol. It performs **no** implicit byte alignment and appends no terminating block. Alignment and `BFINAL` are the caller's decision, and a caller may write several blocks onto one writer.
2. The bit position after a block boundary is exposed through `BitWriter::position()` (`BitPosition::total_bits()`), which is the single source of truth for an orchestrator's width equation.
3. `DeflateBlockPlan::encode` writes all blocks bit-continuously and inserts **no padding between blocks**. A DEFLATE reader looks for the next block header immediately after the last bit of the previous block, so inter-block zero padding would be read as a new header. Only stored blocks align, and that alignment happens inside the block, after its header.
4. The primitives validate **algorithmic expressibility** (length and distance ranges) only. Whether a back-reference points at bytes already produced is orchestrator context; Acorn does not guess it.
5. `distance == 0`, `distance > 32768` and a length outside `3..=258` return a `CodecError` (`InvalidBackReference`, `DistanceOutOfWindow`, `InvalidMatchLength`) carrying the bit position.

## 📐 Limits, Facts and Errors

Every codec that can consume input, produce output or retain state uses one `CodecLimits`:

```text
max_input_bytes   max_output_bytes   max_expansion_ratio   max_steps
max_huffman_symbols   max_distance   max_block_count
```

A transform reads only the fields that apply to it. A format adapter projects its own budget into `CodecLimits`; `acorn-codec` does not know about `Budget`.

`CodecError` is structured so an adapter can map it to a stable diagnostic code without relying on strings. It distinguishes at least: `InputExhausted`, `InvalidByte`, `InvalidPadding`, `InvalidBitPattern`, `IntegerOverflow`, `NonCanonicalEncoding`, `OutputLimitExceeded`, `ExpansionRatioExceeded`, `InvalidBlockHeader`, `InvalidHuffmanTree`, `HuffmanSymbolOutOfRange`, `DistanceOutOfWindow`, `InvalidBackReference`, `InvalidMatchLength`, `ChecksumMismatch`, `InvalidEnvelope`, `TrailingInput`, `UnsupportedFeature` and `InternalInvariantViolation`. Every error carries the algorithm-internal `offset`.

## 🔌 Features

```text
acorn-codec
  default = []           # bit-level, integer and representation transforms are always available
  compression = []       # enables deflate / zlib / gzip
```

A consumer that only needs ZigZag must not link a Deflate implementation. This crate contains no `flate2` dependency and no `flate2` feature name.

## 🏗️ Internal Layout

```text
src/
  lib.rs  error.rs  limits.rs  facts.rs  scalar.rs  slice.rs
  bit/            # BitOrder, BitPosition, BitReader, BitWriter (top level: Huffman and Deflate are consumers)
  integer/        # zigzag, leb128
  representation/ # base64, hex, and future utf transcoding
  compression/    # checksum, stream, deflate/ (decoder, encoder, block, huffman, window), zlib.rs, gzip.rs
```

Dependency direction is `bit → huffman → deflate → zlib / gzip → format adapter`. `zlib.rs` and `gzip.rs` sit beside `deflate/` with no intermediate `envelope/` layer, and each envelope writes its own members.

## 🖥️ Public Surface

```rust
use acorn_codec::{
    bit::{BitOrder, BitPosition, BitReader, BitWriter},
    compression::{Deflate, Gzip, Zlib},
    integer::{Leb128, ZigZag},
    representation::{Base64, Hex},
};
```

## ✅ Ownership

`acorn-codec` fully owns the correctness of raw Deflate, zlib and gzip. An external compression library is never used as a production backend, and tests do not generate samples with one — they use RFC golden vectors, the crate's own encoder, checked-in fixtures and fuzzing.

## 🤝 Contributing

Contributions are welcome! New transforms must fit one of the three families, share `CodecLimits` / `DecodeFacts` / `CodecError`, and keep envelope headers, trailers and checksums inside their own protocol entry.
