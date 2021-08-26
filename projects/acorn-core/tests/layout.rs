use acorn_core::{
    checked_absolute_jump, checked_relative_jump, evidence_absolute, AddressSpaceId, ByteRange,
    JumpError, LayoutGraph, NodeState, RangeError, ReferenceEdge, VisitKey,
};

const FILE: AddressSpaceId = AddressSpaceId(0);

#[test]
fn layout_graph_indexes_nodes_by_start() {
    let mut graph = LayoutGraph::new(FILE, 1_000);
    let header = ByteRange::new(0, 30).expect("header");
    let id = graph
        .insert_node(NodeState::Indexed, header, "local header")
        .expect("insert");
    assert_eq!(graph.node_at_start(0).expect("lookup").id, id);
}

#[test]
fn absolute_jump_rejects_out_of_bounds() {
    let err = checked_absolute_jump(FILE, 100, 90, 20).expect_err("oob");
    assert!(matches!(
        err,
        JumpError::OutOfBounds | JumpError::Range(RangeError::OutOfBounds)
    ));
}

#[test]
fn relative_jump_uses_basis_start() {
    let basis = ByteRange::new(10, 40).expect("basis");
    let target = checked_relative_jump(FILE, 100, basis, 5, 8).expect("jump");
    assert_eq!(target.start, 15);
    assert_eq!(target.len, 8);
}

#[test]
fn walk_with_budget_allows_revisit_cycles_without_infinite_loop() {
    let mut graph = LayoutGraph::new(FILE, 200);
    let a = graph
        .insert_node(NodeState::Indexed, ByteRange::new(0, 10).expect("a"), "a")
        .expect("a");
    let b = graph
        .insert_node(NodeState::Indexed, ByteRange::new(20, 10).expect("b"), "b")
        .expect("b");
    graph.add_edge(ReferenceEdge {
        from: a,
        to: b,
        evidence: evidence_absolute(FILE, "forward", ByteRange::new(20, 10).expect("b")),
    });
    graph.add_edge(ReferenceEdge {
        from: b,
        to: a,
        evidence: evidence_absolute(FILE, "back", ByteRange::new(0, 10).expect("a")),
    });

    let mut visits = 0u32;
    graph
        .walk_with_budget(VisitKey::new(FILE, 0), 4, |_key, _node| {
            visits += 1;
            Ok(())
        })
        .expect("walk");
    assert_eq!(visits, 2);
}
