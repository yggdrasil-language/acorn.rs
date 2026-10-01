/// Progress and validity state of a layout node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeState {
    /// Candidate or anchor discovered during probe.
    Discovered,
    /// Header, directory, or index bounds are recorded.
    Indexed,
    /// Requested fields have been decoded.
    Decoded,
    /// Structural invariants and references validate.
    StructurallyValid,
    /// Semantic invariants validate for the active adapter.
    SemanticallyValid,
    /// Node is intentionally partial and queryable.
    Partial,
    /// Declared content exceeds available bytes.
    Truncated,
    /// Recognized but unsupported by the active adapter.
    Unsupported,
    /// Evidence violates format constraints.
    Invalid,
}

impl NodeState {
    /// Returns whether `next` is a permitted promotion from `self`.
    pub fn can_promote_to(self, next: Self) -> bool {
        use NodeState::*;
        if self == next {
            return true;
        }
        match self {
            Discovered => matches!(
                next,
                Indexed | Decoded | Partial | Truncated | Unsupported | Invalid
            ),
            Indexed => matches!(
                next,
                Decoded | StructurallyValid | Partial | Truncated | Unsupported | Invalid
            ),
            Decoded => matches!(
                next,
                StructurallyValid | SemanticallyValid | Partial | Truncated | Unsupported | Invalid
            ),
            StructurallyValid => matches!(
                next,
                SemanticallyValid | Partial | Truncated | Unsupported | Invalid
            ),
            SemanticallyValid => matches!(next, Partial),
            Partial | Truncated | Unsupported | Invalid => false,
        }
    }
}
