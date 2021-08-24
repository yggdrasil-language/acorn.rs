use acorn_diagnostic::BudgetUsage;

use crate::address::{AddressSpaceId, ByteRange};

/// Why additional bytes are required.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NeedRange {
    /// Address space that must be populated.
    pub space: AddressSpaceId,
    /// Missing byte interval.
    pub range: ByteRange,
    /// Human-readable reason for the request.
    pub reason: String,
}

/// Partial availability of a field, member, or query result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Availability<T> {
    /// Value is present.
    Present(T),
    /// The format definitively has no such field or member.
    Absent,
    /// Current evidence is insufficient. Further reading may resolve it.
    Unknown,
    /// Explicit source ranges are missing and must be supplied.
    NeedRange(NeedRange),
    /// Declared content extends past the available source length.
    Truncated,
    /// Structure is recognizable but unsupported by the active adapter.
    Unsupported,
    /// Parsing stopped because a budget limit was reached.
    ResourceLimited(BudgetUsage),
    /// Read evidence violates format constraints.
    Invalid,
}

impl<T> Availability<T> {
    /// Returns `true` when the value is definitely absent.
    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Returns `true` when more source bytes may change the outcome.
    pub fn may_resolve_with_more_input(&self) -> bool {
        matches!(self, Self::Unknown | Self::NeedRange(_))
    }

    /// Maps a present value without collapsing partial states.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Availability<U> {
        match self {
            Self::Present(value) => Availability::Present(f(value)),
            Self::Absent => Availability::Absent,
            Self::Unknown => Availability::Unknown,
            Self::NeedRange(need) => Availability::NeedRange(need),
            Self::Truncated => Availability::Truncated,
            Self::Unsupported => Availability::Unsupported,
            Self::ResourceLimited(usage) => Availability::ResourceLimited(usage),
            Self::Invalid => Availability::Invalid,
        }
    }
}
