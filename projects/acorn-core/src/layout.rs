use std::collections::BTreeMap;

use crate::address::{AddressSpaceId, ByteRange, RangeError};
use crate::reference::{JumpError, ReferenceEvidence, ReferenceKind, VisitKey};
use crate::state::NodeState;

/// Stable identifier for a layout node within one graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutNodeId(pub u64);

/// One indexed or decoded interval in an address space.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutNode {
    /// Node identity.
    pub id: LayoutNodeId,
    /// Parse progress for this interval.
    pub state: NodeState,
    /// Half-open byte range owned by the node.
    pub range: ByteRange,
    /// Address space containing `range`.
    pub space: AddressSpaceId,
    /// Adapter label, such as `ZIP central directory entry`.
    pub label: String,
}

/// Physical and logical layout index for one address space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutGraph {
    /// Primary address space indexed by this graph.
    pub space: AddressSpaceId,
    /// Known length of the address space, if bounded.
    pub space_len: u64,
    nodes: Vec<LayoutNode>,
    edges: Vec<ReferenceEdge>,
    by_start: BTreeMap<u64, LayoutNodeId>,
}

/// Directed reference between two layout nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReferenceEdge {
    /// Origin node.
    pub from: LayoutNodeId,
    /// Target node when resolved.
    pub to: LayoutNodeId,
    /// Recorded evidence for the jump.
    pub evidence: ReferenceEvidence,
}

impl LayoutGraph {
    /// Creates an empty graph for one address space.
    pub fn new(space: AddressSpaceId, space_len: u64) -> Self {
        Self {
            space,
            space_len,
            nodes: Vec::new(),
            edges: Vec::new(),
            by_start: BTreeMap::new(),
        }
    }

    /// Inserts a node and indexes it by range start offset.
    pub fn insert_node(
        &mut self,
        state: NodeState,
        range: ByteRange,
        label: impl Into<String>,
    ) -> Result<LayoutNodeId, JumpError> {
        range.fits_in(self.space_len)?;
        let id = LayoutNodeId(self.nodes.len() as u64 + 1);
        self.by_start.insert(range.start, id);
        self.nodes.push(LayoutNode {
            id,
            state,
            range,
            space: self.space,
            label: label.into(),
        });
        Ok(id)
    }

    /// Records a resolved reference edge.
    pub fn add_edge(&mut self, edge: ReferenceEdge) {
        self.edges.push(edge);
    }

    /// Resolves an absolute jump inside this graph's address space.
    pub fn resolve_absolute(&self, offset: u64, len: u64) -> Result<ByteRange, JumpError> {
        crate::reference::checked_absolute_jump(self.space, self.space_len, offset, len)
    }

    /// Returns nodes whose ranges overlap `query`.
    pub fn nodes_overlapping(&self, query: ByteRange) -> Result<Vec<&LayoutNode>, RangeError> {
        let mut hits = Vec::new();
        for node in &self.nodes {
            if node.range.overlaps(query)? {
                hits.push(node);
            }
        }
        Ok(hits)
    }

    /// Returns a node by id.
    pub fn node(&self, id: LayoutNodeId) -> Option<&LayoutNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Returns a node starting at `offset`, if indexed.
    pub fn node_at_start(&self, offset: u64) -> Option<&LayoutNode> {
        self.by_start
            .get(&offset)
            .and_then(|id| self.node(*id))
    }

    /// All recorded reference edges.
    pub fn edges(&self) -> &[ReferenceEdge] {
        &self.edges
    }

    /// All layout nodes in insertion order.
    pub fn nodes(&self) -> &[LayoutNode] {
        &self.nodes
    }

    /// Bounded traversal helper that stops on revisits.
    pub fn walk_with_budget<F>(
        &self,
        start: VisitKey,
        max_hops: u32,
        mut visit: F,
    ) -> Result<(), JumpError>
    where
        F: FnMut(VisitKey, &LayoutNode) -> Result<(), JumpError>,
    {
        let mut seen = BTreeMap::new();
        let mut queue = vec![(start, 0u32)];
        while let Some((key, depth)) = queue.pop() {
            if depth > max_hops {
                continue;
            }
            if seen.contains_key(&key) {
                continue;
            }
            seen.insert(key, depth);
            let node = self
                .node_at_start(key.offset)
                .ok_or(JumpError::OutOfBounds)?;
            visit(key, node)?;
            for edge in self.edges.iter().filter(|edge| edge.from == node.id) {
                let target_start = edge.evidence.target.start;
                queue.push((
                    VisitKey::new(edge.evidence.space, target_start),
                    depth + 1,
                ));
            }
        }
        Ok(())
    }
}

/// Convenience builder for reference evidence.
pub fn evidence_absolute(
    space: AddressSpaceId,
    source_field: impl Into<String>,
    target: ByteRange,
) -> ReferenceEvidence {
    ReferenceEvidence {
        kind: ReferenceKind::AbsoluteOffset,
        source_field: source_field.into(),
        basis: None,
        target,
        space,
    }
}
