use acorn_core::ByteRange;
use acorn_source::{ByteSource, MemorySource, ReadOutcome};

use crate::candidate::ProbeCandidate;
use crate::signature::{ProbeAnchor, Signature};

/// Limits for bounded signature search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeBudget {
    pub max_scan_bytes: u64,
    pub max_hits_per_signature: u32,
}

impl Default for ProbeBudget {
    fn default() -> Self {
        Self {
            max_scan_bytes: 64 * 1024,
            max_hits_per_signature: 8,
        }
    }
}

/// Collects probe candidates from registered signatures against one source.
#[derive(Debug, Default)]
pub struct ProbeSession {
    signatures: Vec<Signature>,
    budget: ProbeBudget,
}

impl ProbeSession {
    /// Creates an empty session with default budget.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a signature. Higher `priority` values are listed first.
    pub fn register(&mut self, signature: Signature) {
        self.signatures.push(signature);
        self.signatures
            .sort_by(|left, right| right.priority.cmp(&left.priority));
    }

    /// Overrides the probe budget for this session.
    pub fn with_budget(mut self, budget: ProbeBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Probes `source` and returns all candidates with evidence.
    pub fn probe<S: ByteSource>(&self, source: &S) -> Vec<ProbeCandidate> {
        let source_len = source.length().unwrap_or(0);
        if source_len == 0 {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for signature in &self.signatures {
            self.probe_signature(source, source_len, signature, &mut candidates);
        }
        candidates
    }

    fn probe_signature<S: ByteSource>(
        &self,
        source: &S,
        source_len: u64,
        signature: &Signature,
        out: &mut Vec<ProbeCandidate>,
    ) {
        match signature.anchor {
            ProbeAnchor::Search { start, len } => {
                if signature.evidence_range(source_len).is_none() {
                    return;
                }
                let scan_end = (start + len.min(self.budget.max_scan_bytes)).min(source_len);
                let pattern_len = signature.pattern.bytes.len() as u64;
                let mut hits = 0u32;
                let mut offset = start;
                while offset + pattern_len <= scan_end && hits < self.budget.max_hits_per_signature {
                    if let Some(hit) = self.match_at(source, signature, offset) {
                        out.push(hit);
                        hits += 1;
                    }
                    offset += 1;
                }
            }
            _ => {
                if let Some(hit) = self.match_anchor(source, source_len, signature) {
                    out.push(hit);
                }
            }
        }
    }

    fn match_anchor<S: ByteSource>(
        &self,
        source: &S,
        source_len: u64,
        signature: &Signature,
    ) -> Option<ProbeCandidate> {
        let range = signature.evidence_range(source_len)?;
        let offset = match signature.anchor {
            ProbeAnchor::Start | ProbeAnchor::Fixed(_) => range.start,
            ProbeAnchor::Tail { .. } => range.start,
            ProbeAnchor::Search { .. } => return None,
        };
        self.match_at(source, signature, offset)
    }

    fn match_at<S: ByteSource>(
        &self,
        source: &S,
        signature: &Signature,
        offset: u64,
    ) -> Option<ProbeCandidate> {
        let pattern_len = signature.pattern.bytes.len();
        let mut buffer = vec![0u8; pattern_len];
        match source.read_at(offset, &mut buffer) {
            Ok(ReadOutcome::Complete) | Ok(ReadOutcome::Partial { .. }) => {}
            Ok(ReadOutcome::Eof) | Ok(ReadOutcome::Unavailable { .. }) | Err(_) => return None,
        }
        if !signature.pattern.matches_at(&buffer) {
            return None;
        }
        let evidence = ByteRange {
            start: offset,
            len: pattern_len as u64,
        };
        Some(ProbeCandidate::from_signature_hit(
            signature.format_id.clone(),
            evidence,
            anchor_label(&signature.anchor),
        ))
    }
}

fn anchor_label(anchor: &ProbeAnchor) -> &'static str {
    match anchor {
        ProbeAnchor::Start => "start",
        ProbeAnchor::Fixed(_) => "fixed offset",
        ProbeAnchor::Tail { .. } => "tail window",
        ProbeAnchor::Search { .. } => "search window",
    }
}

/// Built-in signatures useful for container-first document pipelines.
pub fn document_container_signatures() -> Vec<Signature> {
    vec![
        Signature::at_start("zip", b"PK\x03\x04"),
        Signature::at_start("gzip", &[0x1F, 0x8B]),
        Signature::at_start("pdf", b"%PDF-"),
    ]
}

/// Convenience probe of in-memory bytes with built-in container signatures.
pub fn probe_memory(label: impl Into<String>, bytes: Vec<u8>) -> Vec<ProbeCandidate> {
    let source = MemorySource::from_bytes(label, bytes);
    let mut session = ProbeSession::new();
    for signature in document_container_signatures() {
        session.register(signature);
    }
    session.probe(&source)
}
