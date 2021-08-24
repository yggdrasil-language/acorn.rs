use acorn_core::{
    Address, AddressSpaceId, Availability, ByteRange, BudgetTracker, NodeState, ParseBudget,
    Provenance, RangeError, Span,
};

#[test]
fn byte_range_checked_end() {
    let range = ByteRange::new(10, 5).expect("valid range");
    assert_eq!(range.end(), Ok(15));
    assert!(range.fits_in(20).is_ok());
    assert_eq!(range.fits_in(14), Err(RangeError::OutOfBounds));
}

#[test]
fn byte_range_overflow_is_reported() {
    assert_eq!(
        ByteRange::new(u64::MAX, 1),
        Err(RangeError::Overflow)
    );
}

#[test]
fn byte_range_overlap_detection() {
    let left = ByteRange::new(0, 10).expect("left");
    let right = ByteRange::new(5, 10).expect("right");
    assert_eq!(left.overlaps(right), Ok(true));

    let disjoint = ByteRange::new(20, 4).expect("disjoint");
    assert_eq!(left.overlaps(disjoint), Ok(false));
}

#[test]
fn availability_does_not_collapse_unknown_to_absent() {
    let value: Availability<u8> = Availability::Unknown;
    assert!(value.may_resolve_with_more_input());
    assert!(!value.is_absent());
}

#[test]
fn node_state_promotion_is_monotonic() {
    assert!(NodeState::Discovered.can_promote_to(NodeState::Indexed));
    assert!(NodeState::Indexed.can_promote_to(NodeState::StructurallyValid));
    assert!(!NodeState::Invalid.can_promote_to(NodeState::Decoded));
    assert!(!NodeState::SemanticallyValid.can_promote_to(NodeState::Indexed));
}

#[test]
fn budget_tracker_enforces_read_limit() {
    let mut tracker = BudgetTracker::new(ParseBudget {
        max_read_bytes: 16,
        ..ParseBudget::default()
    });
    assert!(tracker.reserve_read_bytes(8).is_ok());
    assert!(tracker.reserve_read_bytes(8).is_ok());
    assert!(tracker.reserve_read_bytes(1).is_err());
}

#[test]
fn provenance_chain_is_preserved() {
    let view = AddressSpaceId(1);
    let outer = ByteRange::new(0, 100).expect("outer");
    let inner = ByteRange::new(0, 40).expect("inner");
    let parent = Provenance::root();
    let span = Span::derived(view, inner, &parent, "ZIP member 3", outer);
    assert_eq!(span.space, view);
    assert_eq!(span.provenance.steps.len(), 1);
    assert_eq!(span.provenance.steps[0].label, "ZIP member 3");
}

#[test]
fn address_equality_is_space_sensitive() {
    let a = Address::new(AddressSpaceId(0), 42);
    let b = Address::new(AddressSpaceId(1), 42);
    assert_ne!(a, b);
}
