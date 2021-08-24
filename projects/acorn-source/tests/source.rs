use acorn_source::{
    ByteSource, MemorySource, RangeMapSource, ReadError, ReadOutcome, SourceRange,
};

#[test]
fn memory_source_reads_without_padding() {
    let source = MemorySource::from_static("sample", b"ABCDE");
    let mut buffer = [0u8; 3];
    let outcome = source.read_at(3, &mut buffer).expect("read succeeds");
    assert_eq!(
        outcome,
        ReadOutcome::Partial { bytes_read: 2 }
    );
    assert_eq!(&buffer[..2], b"DE");
    assert_eq!(buffer[2], 0);
}

#[test]
fn memory_source_reports_eof_without_zero_fill() {
    let source = MemorySource::from_static("sample", b"AB");
    let mut buffer = [9u8; 4];
    let outcome = source.read_at(2, &mut buffer).expect("read succeeds");
    assert_eq!(outcome, ReadOutcome::Eof);
    assert_eq!(buffer, [9, 9, 9, 9]);
}

#[test]
fn partial_source_reports_unavailable_range() {
    let source = RangeMapSource::new("remote", vec![(0, vec![1, 2, 3])]).with_total_length(16);
    let mut buffer = [0u8; 4];
    let outcome = source.read_at(8, &mut buffer).expect("read succeeds");
    assert!(matches!(outcome, ReadOutcome::Unavailable { .. }));
}

#[test]
fn read_range_surfaces_short_read() {
    let source = MemorySource::from_static("sample", b"ABCDE");
    let range = SourceRange::new(0, 8).expect("range");
    let error = source.read_range(range).unwrap_err();
    assert!(matches!(error, ReadError::ShortRead { expected: 8, actual: 5 }));
}

#[test]
fn byte_window_maps_outer_offsets() {
    let source = MemorySource::from_static("sample", b"0123456789");
    let window = acorn_source::ByteWindow::new(
        source,
        0,
        SourceRange::new(2, 4).expect("range"),
    )
    .expect("window");
    let bytes = window
        .read_range(SourceRange::new(0, 4).expect("inner"))
        .expect("read");
    assert_eq!(bytes, b"2345");
}
