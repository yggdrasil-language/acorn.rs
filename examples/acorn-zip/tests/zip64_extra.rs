use acorn_zip::parse_zip64_extra;

#[test]
fn parses_zip64_extra_uncompressed_size() {
    let uncompressed = 0x1_0000_0200u64;
    let mut extra = Vec::new();
    extra.extend_from_slice(&1u16.to_le_bytes());
    extra.extend_from_slice(&8u16.to_le_bytes());
    extra.extend_from_slice(&uncompressed.to_le_bytes());

    let fields = parse_zip64_extra(&extra, 0, extra.len(), true, false, false, 0, 512, 0).expect("zip64 extra");
    assert_eq!(fields.uncompressed_size, uncompressed);
    assert_eq!(fields.compressed_size, 512);
    assert_eq!(fields.local_header_offset, 0);
}
