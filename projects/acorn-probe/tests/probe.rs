use acorn_probe::{probe_memory, Confidence, ProbeSession};
use acorn_source::MemorySource;

#[test]
fn zip_magic_at_start() {
    let bytes = b"PK\x03\x04\x00\x00garbage".to_vec();
    let hits = probe_memory("sample.docx", bytes);
    assert!(hits.iter().any(|hit| hit.format_id == "zip"));
    assert!(hits.iter().all(|hit| hit.confidence == Confidence::Candidate));
}

#[test]
fn pdf_and_gzip_are_distinct_candidates() {
    let pdf = probe_memory("sample.pdf", b"%PDF-1.7\n".to_vec());
    assert!(pdf.iter().any(|hit| hit.format_id == "pdf"));

    let gzip = probe_memory("sample.gz", vec![0x1F, 0x8B, 0x08, 0x00]);
    assert!(gzip.iter().any(|hit| hit.format_id == "gzip"));
}

#[test]
fn tail_anchor_can_match_end_marker() {
    let mut session = ProbeSession::new();
    session.register(acorn_probe::Signature {
        format_id: "zip-eocd".into(),
        anchor: acorn_probe::ProbeAnchor::Tail { len: 22 },
        pattern: acorn_probe::BytePattern::exact(b"PK\x05\x06"),
        priority: 1,
    });
    let mut bytes = vec![0u8; 30];
    bytes.extend_from_slice(b"PK\x05\x06\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");
    let source = MemorySource::from_bytes("zip-tail", bytes);
    let hits = session.probe(&source);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].format_id, "zip-eocd");
}
