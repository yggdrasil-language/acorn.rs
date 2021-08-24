use acorn_diagnostic::BudgetUsage;
use thiserror::Error;

/// Composable limits for parsing work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParseBudget {
    /// Maximum bytes read from raw sources.
    pub max_read_bytes: u64,
    /// Maximum bytes materialized in decoded spaces.
    pub max_decoded_bytes: u64,
    /// Maximum envelope nesting depth.
    pub max_envelope_depth: u32,
    /// Maximum container members indexed in one query.
    pub max_container_members: u64,
    /// Maximum layout nodes allocated.
    pub max_nodes: u64,
    /// Maximum reference edges recorded or followed.
    pub max_references: u64,
    /// Maximum pointer hops during traversal.
    pub max_pointer_hops: u64,
    /// Maximum bytes scanned during signature or window search.
    pub max_scan_bytes: u64,
    /// Maximum bytes retained in caches.
    pub max_cache_bytes: u64,
}

impl Default for ParseBudget {
    fn default() -> Self {
        Self {
            max_read_bytes: 64 * 1024 * 1024,
            max_decoded_bytes: 64 * 1024 * 1024,
            max_envelope_depth: 32,
            max_container_members: 1_000_000,
            max_nodes: 1_000_000,
            max_references: 4_000_000,
            max_pointer_hops: 10_000,
            max_scan_bytes: 8 * 1024 * 1024,
            max_cache_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Errors raised when a budget would be exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BudgetError {
    /// Raw read budget exhausted.
    #[error("read byte budget exhausted")]
    ReadBytes,
    /// Decoded materialization budget exhausted.
    #[error("decoded byte budget exhausted")]
    DecodedBytes,
    /// Envelope depth budget exhausted.
    #[error("envelope depth budget exhausted")]
    EnvelopeDepth,
    /// Container member budget exhausted.
    #[error("container member budget exhausted")]
    ContainerMembers,
    /// Layout node budget exhausted.
    #[error("layout node budget exhausted")]
    Nodes,
    /// Reference budget exhausted.
    #[error("reference budget exhausted")]
    References,
    /// Pointer hop budget exhausted.
    #[error("pointer hop budget exhausted")]
    PointerHops,
    /// Scan budget exhausted.
    #[error("scan byte budget exhausted")]
    ScanBytes,
    /// Cache budget exhausted.
    #[error("cache byte budget exhausted")]
    CacheBytes,
}

/// Tracks budget consumption with checked reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetTracker {
    limits: ParseBudget,
    usage: BudgetUsage,
}

impl BudgetTracker {
    /// Creates a tracker for the given limits.
    pub fn new(limits: ParseBudget) -> Self {
        Self {
            limits,
            usage: BudgetUsage::default(),
        }
    }

    /// Returns the current limits.
    pub const fn limits(&self) -> &ParseBudget {
        &self.limits
    }

    /// Returns the current usage snapshot.
    pub const fn usage(&self) -> &BudgetUsage {
        &self.usage
    }

    /// Reserves additional raw read bytes.
    pub fn reserve_read_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let next = self
            .usage
            .read_bytes
            .checked_add(bytes)
            .ok_or(BudgetError::ReadBytes)?;
        if next > self.limits.max_read_bytes {
            return Err(BudgetError::ReadBytes);
        }
        self.usage.read_bytes = next;
        Ok(())
    }

    /// Reserves additional decoded bytes.
    pub fn reserve_decoded_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let next = self
            .usage
            .decoded_bytes
            .checked_add(bytes)
            .ok_or(BudgetError::DecodedBytes)?;
        if next > self.limits.max_decoded_bytes {
            return Err(BudgetError::DecodedBytes);
        }
        self.usage.decoded_bytes = next;
        Ok(())
    }

    /// Reserves one layout node.
    pub fn reserve_node(&mut self) -> Result<(), BudgetError> {
        let next = self
            .usage
            .nodes
            .checked_add(1)
            .ok_or(BudgetError::Nodes)?;
        if next > self.limits.max_nodes {
            return Err(BudgetError::Nodes);
        }
        self.usage.nodes = next;
        Ok(())
    }

    /// Reserves one pointer hop.
    pub fn reserve_pointer_hop(&mut self) -> Result<(), BudgetError> {
        let next = self
            .usage
            .pointer_hops
            .checked_add(1)
            .ok_or(BudgetError::PointerHops)?;
        if next > self.limits.max_pointer_hops {
            return Err(BudgetError::PointerHops);
        }
        self.usage.pointer_hops = next;
        Ok(())
    }

    /// Reserves one envelope nesting level.
    pub fn reserve_envelope_depth(&mut self, depth: u32) -> Result<(), BudgetError> {
        if depth > self.limits.max_envelope_depth {
            return Err(BudgetError::EnvelopeDepth);
        }
        self.usage.envelope_depth = depth;
        Ok(())
    }
}
