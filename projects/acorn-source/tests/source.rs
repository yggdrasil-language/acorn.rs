use acorn_source::{ByteSource, MemorySource, RangeMapSource, ReadError, ReadOutcome, SourceIdentity, SourceRange};

#[test]
fn memory_source_reads_without_padding() {
    let source = MemorySource::from_static("sample", b"ABCDE");
    let mut buffer = [0u8; 3];
    let outcome = source.read_at(3, &mut buffer).expect("read succeeds");
    assert_eq!(outcome, ReadOutcome::Partial { bytes_read: 2 });
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
    let window = acorn_source::ByteWindow::new(source, 0, SourceRange::new(2, 4).expect("range")).expect("window");
    let bytes = window.read_range(SourceRange::new(0, 4).expect("inner")).expect("read");
    assert_eq!(bytes, b"2345");
}

#[test]
fn content_digest_tracks_content_changes() {
    assert_eq!(acorn_source::content_digest_of(b"ABCDE"), acorn_source::content_digest_of(b"ABCDE"));
    assert_ne!(acorn_source::content_digest_of(b"ABCDE"), acorn_source::content_digest_of(b"ABCDF"));
}

#[test]
fn snapshot_reuse_requires_a_matching_digest() {
    let source = MemorySource::from_static("sample", b"ABCDE");
    let identity = source.identity();

    let same_bytes = MemorySource::from_static("sample", b"ABCDE");
    assert!(identity.snapshot_reuses(&same_bytes.identity()));

    let changed_bytes = MemorySource::from_static("sample", b"ABCDF");
    assert!(!identity.snapshot_reuses(&changed_bytes.identity()));

    let renamed = MemorySource::from_static("moved", b"ABCDE");
    assert!(!identity.snapshot_reuses(&renamed.identity()));

    let labelled_only = SourceIdentity::new("sample");
    assert!(!identity.snapshot_reuses(&labelled_only));
}

#[test]
fn partial_source_identity_cannot_prove_content() {
    let source = RangeMapSource::new("remote", vec![(0, vec![1, 2, 3])]).with_total_length(16);
    let materialized = MemorySource::from_static("remote", b"ABCDE");
    assert!(!source.identity().snapshot_reuses(&materialized.identity()));
    assert!(source.identity().content_digest.is_none());
}

#[test]
fn byte_window_identity_is_not_its_parent_identity() {
    let source = MemorySource::from_static("sample", b"0123456789");
    let parent = source.identity();
    let window = acorn_source::ByteWindow::new(source, 0, SourceRange::new(2, 4).expect("range")).expect("window");
    let windowed = window.identity();

    assert_ne!(windowed.label, parent.label);
    assert!(!windowed.snapshot_reuses(&parent));
}

#[test]
fn supplied_range_satisfies_a_previous_need() {
    let mut source = RangeMapSource::new("remote", vec![(0, vec![1, 2, 3, 4])]).with_total_length(8);
    let mut buffer = [0u8; 4];
    let outcome = source.read_at(4, &mut buffer).expect("read succeeds");
    match outcome {
        ReadOutcome::Unavailable { range, .. } => {
            assert_eq!(range, SourceRange::new(4, 4).expect("range"));
        }
        other => panic!("expected a missing range, got {other:?}"),
    }

    source.provide_range(4, vec![5, 6, 7, 8]).expect("supply the fetched range");
    let outcome = source.read_at(4, &mut buffer).expect("read succeeds");
    assert_eq!(outcome, ReadOutcome::Complete);
    assert_eq!(buffer, [5, 6, 7, 8]);
}

#[test]
fn supplied_range_rejects_contradicting_bytes() {
    let mut source = RangeMapSource::new("remote", vec![(0, vec![1, 2, 3, 4])]).with_total_length(8);
    let error = source.provide_range(2, vec![9, 9]).expect_err("contradiction");
    assert!(matches!(error, ReadError::ConflictingRange { .. }));

    source.provide_range(2, vec![3, 4]).expect("identical overlap is accepted");
    let mut buffer = [0u8; 2];
    assert_eq!(source.read_at(2, &mut buffer).expect("read succeeds"), ReadOutcome::Complete);
    assert_eq!(buffer, [3, 4]);
}

#[test]
fn supplied_range_past_a_known_length_is_rejected() {
    let mut source = RangeMapSource::new("remote", vec![(0, vec![1, 2, 3])]).with_total_length(4);
    let error = source.provide_range(3, vec![4, 5]).expect_err("past the end");
    assert!(matches!(error, ReadError::Unavailable { .. }));
}

#[test]
fn reads_past_a_known_length_report_eof() {
    let source = RangeMapSource::new("remote", vec![(0, vec![1, 2, 3])]).with_total_length(4);
    let mut buffer = [0u8; 2];
    assert_eq!(source.read_at(4, &mut buffer).expect("read succeeds"), ReadOutcome::Eof);
}
