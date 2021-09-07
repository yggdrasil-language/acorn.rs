use acorn_core::{
    AddressSpaceId, Endian, LayoutPlan, PlanError, RelocationTarget,
};

const OUT: AddressSpaceId = AddressSpaceId(1);

#[test]
fn layout_plan_assigns_aligned_offsets() {
    let mut plan = LayoutPlan::new(OUT);
    let header = plan.add_slot("header", 4, 5).expect("header");
    let body = plan.add_slot("body", 8, 16).expect("body");
    let total = plan.finalize().expect("finalize");
    assert_eq!(total, 24);
    assert_eq!(plan.slot(header).expect("header").offset, Some(0));
    assert_eq!(plan.slot(body).expect("body").offset, Some(8));
}

#[test]
fn layout_plan_resolves_slot_relative_relocation() {
    let mut plan = LayoutPlan::new(OUT);
    let body = plan.add_slot("body", 1, 20).expect("body");
    let relocation = plan
        .add_relocation(RelocationTarget::Slot {
            slot: body,
            offset_within: 4,
        })
        .expect("relocation");
    plan.add_patch_point(relocation, body, 0, 4, Endian::Little)
        .expect("patch");
    plan.finalize().expect("finalize");
    let resolved = plan
        .relocations()
        .iter()
        .find(|entry| entry.id == relocation)
        .and_then(|entry| entry.resolved_offset)
        .expect("resolved");
    assert_eq!(resolved, 4);
}

#[test]
fn layout_plan_rejects_invalid_alignment() {
    let mut plan = LayoutPlan::new(OUT);
    let error = plan.add_slot("bad", 3, 8).expect_err("alignment");
    assert!(matches!(
        error,
        PlanError::InvalidAlignment { alignment: 3 }
    ));
}
