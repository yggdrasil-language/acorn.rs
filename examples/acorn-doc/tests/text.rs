use acorn_doc::{decode_piece_table, decode_text_range, fib_clx_location};

#[test]
fn decodes_compressed_piece_table() {
    let mut piece_table = Vec::new();
    piece_table.extend_from_slice(&0u32.to_le_bytes());
    piece_table.extend_from_slice(&3u32.to_le_bytes());
    piece_table.extend_from_slice(&0u16.to_le_bytes());
    piece_table.extend_from_slice(&(0x4000_0009u32).to_le_bytes());
    piece_table.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(decode_piece_table(b"xxxxA\rB", &piece_table, false).as_deref(), Some("A\nB"));
}

#[test]
fn decodes_unicode_piece_table() {
    let mut piece_table = Vec::new();
    piece_table.extend_from_slice(&0u32.to_le_bytes());
    piece_table.extend_from_slice(&2u32.to_le_bytes());
    piece_table.extend_from_slice(&0u16.to_le_bytes());
    piece_table.extend_from_slice(&(4u32).to_le_bytes());
    piece_table.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(decode_piece_table(b"xxxx\x60\x4F\x7D\x59", &piece_table, true).as_deref(), Some("你好"));
}

#[test]
fn decodes_unicode_surrogate_pairs_in_piece_table() {
    let mut piece_table = Vec::new();
    piece_table.extend_from_slice(&0u32.to_le_bytes());
    piece_table.extend_from_slice(&2u32.to_le_bytes());
    piece_table.extend_from_slice(&0u16.to_le_bytes());
    piece_table.extend_from_slice(&(4u32).to_le_bytes());
    piece_table.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(decode_piece_table(b"xxxx=\xD8\x00\xDE", &piece_table, true).as_deref(), Some("😀"));
}

#[test]
fn maps_word_manual_break_controls_to_text_line_breaks() {
    assert_eq!(decode_text_range(b"A\x0BB\x0CC", false), "A\nB\nC");
    assert_eq!(decode_text_range(b"A\x00\x0B\x00\x00\x00\x0C\x00C\x00", true), "A\n\nC");
}

#[test]
fn reads_clx_location_from_fib_layout() {
    let mut fib = vec![0u8; 310];
    fib[32..34].copy_from_slice(&0u16.to_le_bytes());
    fib[34..36].copy_from_slice(&0u16.to_le_bytes());
    fib[36..38].copy_from_slice(&34u16.to_le_bytes());
    fib[38 + 33 * 8..38 + 33 * 8 + 4].copy_from_slice(&123u32.to_le_bytes());
    fib[38 + 33 * 8 + 4..38 + 33 * 8 + 8].copy_from_slice(&456u32.to_le_bytes());
    assert_eq!(fib_clx_location(&fib), Some((123, 456)));
}
