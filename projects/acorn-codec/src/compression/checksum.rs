//! Checksum algorithms for compression envelopes.

/// Computes Adler-32 over `data`.
pub fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Incremental CRC-32 (IEEE / gzip polynomial).
///
/// Streaming envelopes feed decoded chunks as they are produced, so a checksum
/// never needs the whole payload in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    /// Creates a CRC accumulator with the IEEE initial value.
    #[inline]
    pub const fn new() -> Self {
        Self { state: 0xffff_ffff }
    }

    /// Extends the accumulator with `data`.
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.state;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                }
                else {
                    crc >>= 1;
                }
            }
        }
        self.state = crc;
    }

    /// Returns the CRC-32 value of everything fed so far.
    #[inline]
    pub const fn value(&self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes CRC-32 (IEEE / gzip polynomial) over `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(data);
    crc.value()
}
