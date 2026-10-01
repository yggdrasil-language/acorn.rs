use acorn_core::ByteRange;

/// One indexed ZIP member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipMember {
    /// Archive path using forward slashes.
    pub path: String,
    /// Offset of the local file header.
    pub local_header_offset: u64,
    /// Compressed payload size.
    pub compressed_size: u64,
    /// Uncompressed payload size.
    pub uncompressed_size: u64,
    /// Compression method from the central directory.
    pub compression_method: u16,
    /// Byte range of the local header and payload in the archive file space.
    pub data_range: ByteRange,
    /// Byte range of the compressed payload only.
    pub payload_range: ByteRange,
}
