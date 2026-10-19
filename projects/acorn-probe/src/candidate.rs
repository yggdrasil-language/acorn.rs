use acorn_core::ByteRange;

/// Confidence tier for a probe hit. `Candidate` alone is not `Identified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Extension or weak hint only.
    Hint,
    /// Magic or signature matched at the declared anchor.
    Candidate,
    /// Reserved for a later structural validation phase.
    Identified,
}

/// One format hypothesis with evidence ranges and human-readable reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeCandidate {
    /// Format identifier registered with the matching signature.
    pub format_id: String,
    /// Confidence tier of this hypothesis.
    pub confidence: Confidence,
    /// Byte ranges that produced the evidence, in source coordinates.
    pub evidence_ranges: Vec<ByteRange>,
    /// Optional version hint extracted or declared by the format.
    pub version_hint: Option<String>,
    /// Human-readable reasons supporting this candidate.
    pub reasons: Vec<String>,
}

impl ProbeCandidate {
    pub(crate) fn from_signature_hit(format_id: String, evidence: ByteRange, anchor_label: &str) -> Self {
        Self {
            format_id,
            confidence: Confidence::Candidate,
            evidence_ranges: vec![evidence],
            version_hint: None,
            reasons: vec![format!("matched signature at {}", anchor_label)],
        }
    }
}
