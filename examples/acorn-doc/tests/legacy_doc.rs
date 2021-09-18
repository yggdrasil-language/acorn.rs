use std::io::{Cursor, Write};

use acorn_doc::{extract_text_from_bytes, open_legacy_doc};
use cfb::CompoundFile as OracleCompoundFile;

fn minimal_legacy_doc_bytes(text: &str) -> Vec<u8> {
    let mut word_document = vec![0u8; 32];
    word_document[0..2].copy_from_slice(&[0xec, 0xa5]);
    let text_bytes = text.as_bytes();
    let fc_min = 32u32;
    let fc_mac = fc_min + text_bytes.len() as u32;
    word_document.extend_from_slice(text_bytes);
    word_document[24..28].copy_from_slice(&fc_min.to_le_bytes());
    word_document[28..32].copy_from_slice(&fc_mac.to_le_bytes());

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut compound = OracleCompoundFile::create(&mut buffer).expect("create compound file");
        {
            let mut stream = compound
                .create_stream("/WordDocument")
                .expect("WordDocument stream");
            stream.write_all(&word_document).expect("write WordDocument");
        }
        {
            let mut stream = compound.create_stream("/0Table").expect("0Table stream");
            stream.write_all(&[]).expect("write 0Table");
        }
    }
    buffer.into_inner()
}

#[test]
fn extracts_simple_legacy_doc_text() {
    let bytes = minimal_legacy_doc_bytes("Hello legacy DOC\rSecond paragraph");
    let extracted = extract_text_from_bytes(&bytes).expect("extract text");
    assert!(extracted.text.contains("Hello legacy DOC"));
    assert!(extracted.text.contains("Second paragraph"));
    assert!(!extracted.complex);
    assert!(!extracted.piece_table_used);
}

#[test]
fn rejects_non_compound_input() {
    let error = open_legacy_doc(b"plain text").expect_err("expected error");
    assert!(matches!(error, acorn_doc::DocError::NotCompoundFile));
}
