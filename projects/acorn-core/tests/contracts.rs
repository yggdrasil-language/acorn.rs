use acorn_core::{
    Address, AddressSpaceId, Availability, Budget, BudgetTracker, ByteRange, NodeState, Provenance, RangeError, Span,
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
    assert_eq!(ByteRange::new(u64::MAX, 1), Err(RangeError::Overflow));
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
    let mut tracker = BudgetTracker::new(Budget { max_read_bytes: 16, ..Budget::default() });
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

#[test]
fn reserved_read_bytes_are_held_until_settled() {
    let mut tracker = BudgetTracker::new(Budget { max_read_bytes: 16, ..Budget::default() });
    tracker.reserve_read_bytes(16).expect("reserve the whole limit");
    assert_eq!(tracker.reserved_read_bytes(), 16);
    assert_eq!(tracker.usage().read_bytes, 0);
    assert!(tracker.reserve_read_bytes(1).is_err());

    tracker.settle_read_bytes(10).expect("settle the bytes that were read");
    assert_eq!(tracker.usage().read_bytes, 10);
    assert_eq!(tracker.reserved_read_bytes(), 6);
    assert!(tracker.settle_read_bytes(7).is_err());

    tracker.settle_read_bytes(6).expect("settle the rest");
    assert_eq!(tracker.usage().read_bytes, 16);
    assert_eq!(tracker.reserved_read_bytes(), 0);
}

#[test]
fn released_read_bytes_return_to_the_allowance() {
    let mut tracker = BudgetTracker::new(Budget { max_read_bytes: 16, ..Budget::default() });
    tracker.reserve_read_bytes(16).expect("reserve everything");
    tracker.release_read_bytes(6).expect("release the unused hold");
    assert_eq!(tracker.reserved_read_bytes(), 10);
    assert!(tracker.release_read_bytes(11).is_err());
    tracker.reserve_read_bytes(6).expect("the released allowance is available again");
}

#[test]
fn decoded_bytes_keep_their_own_ledger() {
    let mut tracker = BudgetTracker::new(Budget { max_read_bytes: 16, max_decoded_bytes: 8, ..Budget::default() });
    tracker.reserve_decoded_bytes(8).expect("reserve the decode");
    assert!(tracker.reserve_decoded_bytes(1).is_err());
    tracker.settle_decoded_bytes(5).expect("settle the decode");
    assert_eq!(tracker.usage().decoded_bytes, 5);
    assert_eq!(tracker.reserved_decoded_bytes(), 3);
    assert_eq!(tracker.usage().read_bytes, 0);
}

#[test]
fn retained_bytes_track_a_peak_apart_from_work() {
    let mut tracker = BudgetTracker::new(Budget { max_cache_bytes: 10, ..Budget::default() });
    tracker.reserve_cache_bytes(6).expect("first retain");
    tracker.reserve_cache_bytes(4).expect("second retain");
    assert_eq!(tracker.peak_cache_bytes(), 10);
    assert!(tracker.reserve_cache_bytes(1).is_err());

    tracker.release_cache_bytes(7).expect("release retained bytes");
    assert_eq!(tracker.usage().cache_bytes, 3);
    assert_eq!(tracker.peak_cache_bytes(), 10);
    assert!(tracker.release_cache_bytes(4).is_err());
}

#[test]
fn memory_limits_record_members_references_and_scans() {
    let mut tracker =
        BudgetTracker::new(Budget { max_container_members: 1, max_references: 1, max_scan_bytes: 4, ..Budget::default() });
    tracker.reserve_container_member().expect("first member");
    assert!(tracker.reserve_container_member().is_err());
    tracker.reserve_reference().expect("first reference");
    assert!(tracker.reserve_reference().is_err());
    tracker.reserve_scan_bytes(4).expect("scan the limit");
    assert!(tracker.reserve_scan_bytes(1).is_err());
}
