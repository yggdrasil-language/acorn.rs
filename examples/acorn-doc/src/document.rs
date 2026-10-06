use acorn_cfb::CompoundFile;

use crate::error::{DocError, Result};
use crate::text::extract_word_text;

/// Opened legacy Word document streams.
#[derive(Debug, Clone)]
pub struct LegacyDoc {
    word_document: Vec<u8>,
    table_stream: Vec<u8>,
}

/// Plain text extracted from a legacy Word document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedText {
    /// Decoded document body text.
    pub text: String,
    /// Whether the FIB marks the document as complex.
    pub complex: bool,
    /// Whether CLX piece-table reconstruction was used.
    pub piece_table_used: bool,
}

/// Opens a legacy `.doc` compound file from bytes.
pub fn open_legacy_doc(bytes: &[u8]) -> Result<LegacyDoc> {
    if !CompoundFile::is_compound_file(bytes) {
        return Err(DocError::NotCompoundFile);
    }
    let compound = CompoundFile::open(bytes.to_vec()).map_err(|error| DocError::CompoundFile(error.to_string()))?;
    let word_document = compound
        .read_stream("/WordDocument")
        .map_err(|error| DocError::Stream {
            stream: "/WordDocument".to_string(),
            message: error.to_string(),
        })?;
    let table_stream = read_table_stream(&compound, &word_document).unwrap_or_default();
    Ok(LegacyDoc {
        word_document,
        table_stream,
    })
}

/// Opens and extracts plain text from legacy `.doc` bytes.
pub fn extract_text_from_bytes(bytes: &[u8]) -> Result<ExtractedText> {
    open_legacy_doc(bytes)?.extract_text()
}

impl LegacyDoc {
    /// Extracts plain text from the opened Word streams.
    pub fn extract_text(&self) -> Result<ExtractedText> {
        let (text, complex, piece_table_used) =
            extract_word_text(&self.word_document, &self.table_stream)?;
        Ok(ExtractedText {
            text,
            complex,
            piece_table_used,
        })
    }
}

fn read_table_stream(compound: &CompoundFile, word_document: &[u8]) -> Result<Vec<u8>> {
    let flags = word_document
        .get(10..12)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
        .unwrap_or_default();
    let stream_name = if flags & (1 << 9) != 0 {
        "/1Table"
    } else {
        "/0Table"
    };
    compound
        .read_stream(stream_name)
        .map_err(|error| DocError::Stream {
            stream: stream_name.to_string(),
            message: error.to_string(),
        })
}
