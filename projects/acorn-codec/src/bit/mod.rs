//! Bit-level input and output primitives.
//!
//! The `bit` module owns bit cursors over finite buffers. Huffman coding,
//! deflate blocks, packed fields, and future bit-oriented formats all consume
//! these primitives, so the module never depends on any of them.
//!
//! Bit order is always explicit:
//!
//! - [`BitOrder::LeastSignificantBitFirst`] consumes each byte starting at its
//!   least significant bit. Deflate and gzip use this order.
//! - [`BitOrder::MostSignificantBitFirst`] consumes each byte starting at its
//!   most significant bit.
//!
//! [`BitReader::read_bits`] returns the requested field as a number in stream
//! order: the first bit read becomes bit 0 for least-significant-first order,
//! and the most significant bit for most-significant-first order. Positions are
//! reported as [`BitPosition`] values counted from the start of the buffer.

mod order;
mod position;
mod reader;
mod writer;

pub use self::{
    order::BitOrder,
    position::BitPosition,
    reader::{BitReader, MAX_READ_BITS},
    writer::{BitWriter, MAX_WRITE_BITS},
};
