use acorn_diagnostic::{Diagnostic, DiagnosticCode, RecoveryAction, Severity};

#[test]
fn diagnostic_codes_are_stable_strings() {
    assert_eq!(
        DiagnosticCode::NeedRange.as_str(),
        "ACORN_NEED_RANGE"
    );
    assert_eq!(
        DiagnosticCode::OffsetOverflow.as_str(),
        "ACORN_OFFSET_OVERFLOW"
    );
}

#[test]
fn diagnostic_builder_attaches_params_and_recovery() {
    let diagnostic = Diagnostic::error(DiagnosticCode::Truncated, "input ended early")
        .with_param("expected", "64")
        .with_recovery(RecoveryAction::ProvideRanges);

    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.params.get("expected"), Some(&"64".to_string()));
    assert_eq!(diagnostic.recovery, vec![RecoveryAction::ProvideRanges]);
}
