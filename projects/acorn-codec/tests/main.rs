#[path = "base64.rs"]
mod base64;
#[cfg(feature = "compression")]
#[path = "deflate.rs"]
mod deflate;
#[cfg(feature = "compression")]
#[path = "gzip.rs"]
mod gzip;
#[path = "hex.rs"]
mod hex;
#[path = "varint.rs"]
mod varint;
#[path = "zigzag.rs"]
mod zigzag;
#[cfg(feature = "compression")]
#[path = "zlib.rs"]
mod zlib;
