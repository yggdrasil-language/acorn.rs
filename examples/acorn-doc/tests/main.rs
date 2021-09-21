//! Heavy conformance fixtures and cross-format probes live downstream:
//!
//! - [notedown-fixtures](https://github.com/notedge/notedown/tree/dev/projects/crates/notedown-fixtures/tests/acorn/doc/)
//!
//! **Do NOT** add checked-in binary fixtures or golden tests here.

use acorn_doc::{DocError, open_legacy_doc};

#[test]
fn smoke() {
    let error = open_legacy_doc(b"plain text").expect_err("expected non-compound input error");
    assert!(matches!(error, DocError::NotCompoundFile));
}
