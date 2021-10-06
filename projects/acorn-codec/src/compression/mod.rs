//! Stream compression transforms.
//!
//! Deflate is the algorithm. Zlib and gzip are different Deflate envelopes that
//! share the same core, so they live in `envelope` and never re-implement the
//! bitstream. ZIP is an archive format with members, a central directory, and
//! its own CRC rules, so it stays in `acorn-zip` and only borrows
//! [`RawDeflate`] for compression method 8.

mod checksum;
pub mod deflate;
pub mod envelope;
mod stream;

pub use self::{
    deflate::Deflate,
    envelope::{Gzip, RawDeflate, Zlib},
    stream::{CompressionProtocol, StreamCodec, StreamDecoder},
};
