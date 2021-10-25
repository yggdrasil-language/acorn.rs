//! CRC-32 checksums used by ZIP members.

/// Computes the CRC-32 value defined by IEEE 802.3 and used by ZIP.
pub fn ieee_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
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
    !crc
}
