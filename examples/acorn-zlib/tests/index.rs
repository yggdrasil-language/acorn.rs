use acorn_codec::compression::envelope::zlib::encode_stored;
use acorn_zlib::index_zlib_bytes;

#[test]
fn indexes_minimal_zlib_layout() {
    let bytes = encode_stored(b"x");
    let index = index_zlib_bytes(bytes).expect("index");
    assert_eq!(index.header.header_size, 2);
    assert!(index.header.member_range.len > 4);
    assert_eq!(index.layout.nodes().len(), 2);
}
