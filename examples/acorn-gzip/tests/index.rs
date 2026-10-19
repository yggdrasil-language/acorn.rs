use acorn_codec::compression::envelope::gzip::encode_stored;
use acorn_gzip::index_gzip_bytes;

#[test]
fn indexes_minimal_gzip_layout() {
    let bytes = encode_stored(b"x");
    let index = index_gzip_bytes(bytes).expect("index");
    assert!(index.header.header_size >= 10);
    assert!(index.header.member_range.len > 8);
    assert_eq!(index.layout.nodes().len(), 2);
}
