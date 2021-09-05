use acorn_core::diagnostic::{
    actions, error, warning, BudgetUsage, DiagnosticCode, DiagnosticSeverity, DiagnosticSet,
    MessageArg,
};

#[test]
fn diagnostic_codes_use_dotted_wire_identifiers() {
    assert_eq!(DiagnosticCode::NeedRange.as_str(), "acorn.layout.need-range");
    assert_eq!(
        DiagnosticCode::OffsetOverflow.as_str(),
        "acorn.layout.offset-overflow"
    );
    assert_eq!(
        DiagnosticCode::MagicMismatch.as_str(),
        "acorn.probe.magic-mismatch"
    );
}

#[test]
fn builders_emit_unified_diagnostic_records() {
    let diagnostic = error(DiagnosticCode::Truncated, "input ended early")
        .with_action(actions::provide_ranges().with_arg("expected", MessageArg::U64(64)));

    assert_eq!(diagnostic.code().as_str(), "acorn.layout.truncated");
    assert_eq!(diagnostic.severity(), DiagnosticSeverity::Error);
    assert_eq!(diagnostic.origin().namespace(), "acorn");
    assert_eq!(diagnostic.message().fallback(), Some("input ended early"));
    assert_eq!(diagnostic.actions().len(), 1);
}

#[test]
fn diagnostic_set_collects_acorn_records() {
    let mut set = DiagnosticSet::new();
    set.push(warning(DiagnosticCode::NeedRange, "need tail bytes"));
    assert_eq!(set.diagnostics().len(), 1);
}

#[test]
fn budget_usage_defaults_to_zero() {
    assert_eq!(BudgetUsage::default().read_bytes, 0);
}
