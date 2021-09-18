use crate::error::{DocError, Result};

pub(crate) fn extract_word_text(bytes: &[u8], table_stream: &[u8]) -> Result<(String, bool, bool)> {
    if bytes.len() >= 32 && bytes[..2] == [0xec, 0xa5] {
        let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
        if flags & 0x8100 != 0 {
            return Err(DocError::Encrypted);
        }
        let fc_min = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
        let fc_mac = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
        if fc_min < fc_mac && fc_mac <= bytes.len() {
            let unicode = flags & (1 << 12) != 0;
            let complex = flags & (1 << 2) != 0;
            if complex {
                if let Some(text) = decode_complex_text(bytes, table_stream, unicode) {
                    return Ok((text, true, true));
                }
            }
            return Ok((decode_text_range(&bytes[fc_min..fc_mac], unicode), complex, false));
        }
    }
    Err(DocError::InvalidFib)
}

fn decode_complex_text(word_document: &[u8], table_stream: &[u8], default_unicode: bool) -> Option<String> {
    let (fc_clx, lcb_clx) = fib_clx_location(word_document)?;
    let clx_end = fc_clx.checked_add(lcb_clx)?;
    let clx = table_stream.get(fc_clx..clx_end)?;
    let mut cursor = 0;
    while cursor < clx.len() {
        match clx[cursor] {
            0x01 => {
                let size = u16::from_le_bytes(clx.get(cursor + 1..cursor + 3)?.try_into().ok()?) as usize;
                cursor = cursor.checked_add(3 + size)?;
            }
            0x02 => {
                let size = u32::from_le_bytes(clx.get(cursor + 1..cursor + 5)?.try_into().ok()?) as usize;
                let piece_table = clx.get(cursor + 5..cursor.checked_add(5 + size)?)?;
                return decode_piece_table(word_document, piece_table, default_unicode);
            }
            _ => return None,
        }
    }
    None
}

fn fib_clx_location(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.len() < 34 {
        return None;
    }
    let csw = u16::from_le_bytes(bytes[32..34].try_into().ok()?) as usize;
    let mut cursor = 34usize.checked_add(csw.checked_mul(2)?)?;
    let cslw = u16::from_le_bytes(bytes.get(cursor..cursor + 2)?.try_into().ok()?) as usize;
    cursor = cursor.checked_add(2)?.checked_add(cslw.checked_mul(4)?)?;
    let count = u16::from_le_bytes(bytes.get(cursor..cursor + 2)?.try_into().ok()?) as usize;
    if count <= 33 {
        return None;
    }
    let field = cursor.checked_add(2)?.checked_add(33usize.checked_mul(8)?)?;
    Some((
        u32::from_le_bytes(bytes.get(field..field + 4)?.try_into().ok()?) as usize,
        u32::from_le_bytes(bytes.get(field + 4..field + 8)?.try_into().ok()?) as usize,
    ))
}

pub(crate) fn decode_piece_table(word_document: &[u8], piece_table: &[u8], default_unicode: bool) -> Option<String> {
    if piece_table.len() < 4 || (piece_table.len() - 4) % 12 != 0 {
        return None;
    }
    let piece_count = (piece_table.len() - 4) / 12;
    let cp_bytes = piece_count.checked_add(1)?.checked_mul(4)?;
    let mut output = String::new();
    for index in 0..piece_count {
        let cp_start = u32::from_le_bytes(piece_table[index * 4..index * 4 + 4].try_into().ok()?);
        let cp_end = u32::from_le_bytes(piece_table[(index + 1) * 4..(index + 1) * 4 + 4].try_into().ok()?);
        if cp_end < cp_start {
            return None;
        }
        let pcd = cp_bytes + index * 8;
        let fc_raw = u32::from_le_bytes(piece_table[pcd + 2..pcd + 6].try_into().ok()?);
        let compressed = fc_raw & 0x4000_0000 != 0;
        let offset = if compressed {
            (fc_raw & !0x4000_0000) / 2
        } else {
            fc_raw
        } as usize;
        let character_count = (cp_end - cp_start) as usize;
        if compressed {
            let end = offset.checked_add(character_count)?;
            output.push_str(&decode_compressed_range(word_document.get(offset..end)?));
        } else {
            let end = offset.checked_add(character_count.checked_mul(2)?)?;
            output.push_str(&decode_text_range(word_document.get(offset..end)?, true));
        }
    }
    if output.is_empty() && default_unicode {
        return None;
    }
    Some(output)
}

fn decode_compressed_range(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| match byte {
            0x80 => '€',
            0x82 => '‚',
            0x83 => 'ƒ',
            0x84 => '„',
            0x85 => '…',
            0x86 => '†',
            0x87 => '‡',
            0x88 => 'ˆ',
            0x89 => '‰',
            0x8a => 'Š',
            0x8b => '‹',
            0x8c => 'Œ',
            0x8e => 'Ž',
            0x91 => '‘',
            0x92 => '’',
            0x93 => '“',
            0x94 => '”',
            0x95 => '•',
            0x96 => '–',
            0x97 => '—',
            0x98 => '˜',
            0x99 => '™',
            0x9a => 'š',
            0x9b => '›',
            0x9c => 'œ',
            0x9e => 'ž',
            0x9f => 'Ÿ',
            0x0d | 0x0b | 0x0c => '\n',
            0x09 => '\t',
            0x00..=0x7f | 0xa0..=0xff => *byte as char,
            _ => '\u{FFFD}',
        })
        .collect()
}

pub(crate) fn decode_text_range(bytes: &[u8], unicode: bool) -> String {
    let mut output = String::new();
    if unicode {
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
        for decoded in char::decode_utf16(units) {
            let character = decoded.unwrap_or('\u{FFFD}');
            match character {
                '\r' | '\u{000B}' | '\u{000C}' => output.push('\n'),
                '\t' => output.push('\t'),
                character if !character.is_control() => output.push(character),
                _ => {}
            }
        }
    } else {
        for byte in bytes {
            match byte {
                0x0d | 0x0b | 0x0c => output.push('\n'),
                0x09 => output.push('\t'),
                0x20..=0x7e | 0xa0..=0xff => output.push(*byte as char),
                _ => {}
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{decode_piece_table, decode_text_range, fib_clx_location};

    #[test]
    fn decodes_compressed_piece_table() {
        let mut piece_table = Vec::new();
        piece_table.extend_from_slice(&0u32.to_le_bytes());
        piece_table.extend_from_slice(&3u32.to_le_bytes());
        piece_table.extend_from_slice(&0u16.to_le_bytes());
        piece_table.extend_from_slice(&(0x4000_0009u32).to_le_bytes());
        piece_table.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            decode_piece_table(b"xxxxA\rB", &piece_table, false).as_deref(),
            Some("A\nB")
        );
    }

    #[test]
    fn decodes_unicode_piece_table() {
        let mut piece_table = Vec::new();
        piece_table.extend_from_slice(&0u32.to_le_bytes());
        piece_table.extend_from_slice(&2u32.to_le_bytes());
        piece_table.extend_from_slice(&0u16.to_le_bytes());
        piece_table.extend_from_slice(&(4u32).to_le_bytes());
        piece_table.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            decode_piece_table(b"xxxx\x60\x4F\x7D\x59", &piece_table, true).as_deref(),
            Some("你好")
        );
    }

    #[test]
    fn decodes_unicode_surrogate_pairs_in_piece_table() {
        let mut piece_table = Vec::new();
        piece_table.extend_from_slice(&0u32.to_le_bytes());
        piece_table.extend_from_slice(&2u32.to_le_bytes());
        piece_table.extend_from_slice(&0u16.to_le_bytes());
        piece_table.extend_from_slice(&(4u32).to_le_bytes());
        piece_table.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            decode_piece_table(b"xxxx=\xD8\x00\xDE", &piece_table, true).as_deref(),
            Some("😀")
        );
    }

    #[test]
    fn maps_word_manual_break_controls_to_text_line_breaks() {
        assert_eq!(decode_text_range(b"A\x0BB\x0CC", false), "A\nB\nC");
        assert_eq!(
            decode_text_range(b"A\x00\x0B\x00\x00\x00\x0C\x00C\x00", true),
            "A\n\nC"
        );
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
}
