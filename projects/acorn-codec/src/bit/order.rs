//! Bit order inside a byte.

/// Order in which bits are consumed from or emitted into a byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitOrder {
    /// Bits are consumed starting at the least significant bit of each byte.
    ///
    /// Deflate and gzip use this order.
    #[default]
    LeastSignificantBitFirst,
    /// Bits are consumed starting at the most significant bit of each byte.
    MostSignificantBitFirst,
}
