use acorn_source::{ByteSink, MemorySink, WriteError, WriteOutcome};

#[test]
fn memory_sink_appends_and_patches() {
    let mut sink = MemorySink::new();
    assert_eq!(
        sink.append(b"hello").expect("append"),
        WriteOutcome::Complete
    );
    sink.write_at(0, b"HELLO").expect("write_at");
    sink.patch_u32_le(5, 0x0102_0304).expect("patch");
    assert_eq!(sink.bytes(), b"HELLO\x04\x03\x02\x01");
}

#[test]
fn memory_sink_enforces_budget_on_append() {
    let mut sink = MemorySink::with_budget(4);
    assert_eq!(
        sink.append(b"abc").expect("append"),
        WriteOutcome::Complete
    );
    assert_eq!(
        sink.append(b"de").expect("append"),
        WriteOutcome::WouldExceedBudget {
            attempted: 5,
            limit: 4,
        }
    );
}

#[test]
fn memory_sink_enforces_budget_on_seek_write() {
    let mut sink = MemorySink::with_budget(4);
    let error = sink.write_at(2, b"abcd").expect_err("budget");
    assert!(matches!(
        error,
        WriteError::BudgetExceeded {
            attempted: 6,
            limit: 4
        }
    ));
}
