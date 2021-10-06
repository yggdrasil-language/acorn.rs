//! Deflate envelopes.
//!
//! Each envelope declares its own header, trailer, and checksum rules around
//! the shared [`crate::compression::Deflate`] core. Callers must name the
//! protocol they expect: no entry guesses the envelope from the input.

pub mod gzip;
mod raw;
pub mod zlib;

pub use self::{gzip::Gzip, raw::RawDeflate, zlib::Zlib};
