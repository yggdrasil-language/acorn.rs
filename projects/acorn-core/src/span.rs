use crate::address::{AddressSpaceId, ByteRange};

/// One step in provenance from an outer span to an inner view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProvenanceStep {
    /// Human-readable description, such as `ZIP member 3`.
    pub label: String,
    /// Outer range that produced this step.
    pub outer_range: ByteRange,
}

/// How a span was obtained from outer inputs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Provenance {
    /// Ordered chain from the original file to this span.
    pub steps: Vec<ProvenanceStep>,
}

impl Provenance {
    /// Creates provenance with no steps.
    pub fn root() -> Self {
        Self::default()
    }

    /// Extends the chain with another transformation step.
    pub fn push(&mut self, label: impl Into<String>, outer_range: ByteRange) {
        self.steps.push(ProvenanceStep {
            label: label.into(),
            outer_range,
        });
    }
}

/// A located byte interval with address space and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    /// Address space containing the range.
    pub space: AddressSpaceId,
    /// Byte interval inside the space.
    pub range: ByteRange,
    /// How this span was derived.
    pub provenance: Provenance,
}

impl Span {
    /// Creates a root span in the given space.
    pub fn new(space: AddressSpaceId, range: ByteRange) -> Self {
        Self {
            space,
            range,
            provenance: Provenance::root(),
        }
    }

    /// Creates a child span with extended provenance.
    pub fn derived(
        space: AddressSpaceId,
        range: ByteRange,
        parent: &Provenance,
        label: impl Into<String>,
        outer_range: ByteRange,
    ) -> Self {
        let mut provenance = parent.clone();
        provenance.push(label, outer_range);
        Self {
            space,
            range,
            provenance,
        }
    }
}
