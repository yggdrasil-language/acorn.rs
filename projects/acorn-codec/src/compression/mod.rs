//! Stream compression: one algorithm and its two envelopes.
//!
//! [`Deflate`] is the RFC 1951 algorithm itself. It has no header, no trailer,
//! no checksum, and no member or container concept, so raw Deflate is simply
//! `Deflate` and needs no parallel entry of its own.
//!
//! [`Zlib`] and [`Gzip`] are envelopes of that algorithm. Each owns its own
//! header, trailer, checksum, window declaration, and trailing-input rule, and
//! delegates the bitstream to [`Deflate`].
//!
//! ZIP is not a Deflate envelope. It is a container format with members, a
//! central directory, and its own CRC rules, so it lives in `acorn-zip` and only
//! borrows [`Deflate`] for compression method 8.

mod checksum;
pub mod deflate;
pub mod gzip;
mod stream;
pub mod zlib;

pub use self::{
    deflate::Deflate,
    gzip::Gzip,
    stream::{CompressionProtocol, StreamCodec, StreamDecoder},
    zlib::Zlib,
};
