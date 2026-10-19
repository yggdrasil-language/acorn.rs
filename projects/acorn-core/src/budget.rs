use std::fmt;

use crate::diagnostic::BudgetUsage;

/// Composable limits for parsing work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Budget {
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

impl Default for Budget {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    /// Raw read budget exhausted.
    ReadBytes,
    /// Decoded materialization budget exhausted.
    DecodedBytes,
    /// Envelope depth budget exhausted.
    EnvelopeDepth,
    /// Container member budget exhausted.
    ContainerMembers,
    /// Layout node budget exhausted.
    Nodes,
    /// Reference budget exhausted.
    References,
    /// Pointer hop budget exhausted.
    PointerHops,
    /// Scan budget exhausted.
    ScanBytes,
    /// Cache budget exhausted.
    CacheBytes,
    /// Settlement or release exceeded the bytes that were reserved.
    Unreserved,
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadBytes => write!(f, "read byte budget exhausted"),
            Self::DecodedBytes => write!(f, "decoded byte budget exhausted"),
            Self::EnvelopeDepth => write!(f, "envelope depth budget exhausted"),
            Self::ContainerMembers => write!(f, "container member budget exhausted"),
            Self::Nodes => write!(f, "layout node budget exhausted"),
            Self::References => write!(f, "reference budget exhausted"),
            Self::PointerHops => write!(f, "pointer hop budget exhausted"),
            Self::ScanBytes => write!(f, "scan byte budget exhausted"),
            Self::CacheBytes => write!(f, "cache byte budget exhausted"),
            Self::Unreserved => write!(f, "settlement or release exceeded the reserved bytes"),
        }
    }
}

impl std::error::Error for BudgetError {}

/// Tracks budget consumption with checked reservation, settlement, and release.
///
/// Settled consumption is cumulative work and cannot be given back. Reserved bytes
/// are held for work that has not run yet and count against the limit until they are
/// settled or released. Retained bytes are live memory, and their high-water mark is
/// kept apart from cumulative reads and decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetTracker {
    limits: Budget,
    usage: BudgetUsage,
    reserved_read_bytes: u64,
    reserved_decoded_bytes: u64,
    peak_cache_bytes: u64,
}

impl BudgetTracker {
    /// Creates a tracker for the given limits.
    pub fn new(limits: Budget) -> Self {
        Self { limits, usage: BudgetUsage::default(), reserved_read_bytes: 0, reserved_decoded_bytes: 0, peak_cache_bytes: 0 }
    }

    /// Returns the current limits.
    pub const fn limits(&self) -> &Budget {
        &self.limits
    }

    /// Returns the current usage snapshot.
    pub const fn usage(&self) -> &BudgetUsage {
        &self.usage
    }

    /// Returns read bytes that are reserved but not yet settled.
    pub const fn reserved_read_bytes(&self) -> u64 {
        self.reserved_read_bytes
    }

    /// Returns decoded bytes that are reserved but not yet settled.
    pub const fn reserved_decoded_bytes(&self) -> u64 {
        self.reserved_decoded_bytes
    }

    /// Returns the largest retained byte count observed so far.
    pub const fn peak_cache_bytes(&self) -> u64 {
        self.peak_cache_bytes
    }

    /// Reserves raw read bytes for work that has not read them yet.
    ///
    /// Outstanding reservations count against the limit together with settled
    /// consumption, so two callers cannot hold the same allowance at the same time.
    pub fn reserve_read_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let held = self
            .usage
            .read_bytes
            .checked_add(self.reserved_read_bytes)
            .and_then(|held| held.checked_add(bytes))
            .ok_or(BudgetError::ReadBytes)?;
        if held > self.limits.max_read_bytes {
            return Err(BudgetError::ReadBytes);
        }
        self.reserved_read_bytes = held - self.usage.read_bytes;
        Ok(())
    }

    /// Records the bytes a reserved read actually consumed.
    ///
    /// `actual` must fit inside the outstanding reservation. Held bytes that the read
    /// did not need stay reserved until they are released.
    pub fn settle_read_bytes(&mut self, actual: u64) -> Result<(), BudgetError> {
        let remaining = self.reserved_read_bytes.checked_sub(actual).ok_or(BudgetError::Unreserved)?;
        let consumed = self.usage.read_bytes.checked_add(actual).ok_or(BudgetError::ReadBytes)?;
        if consumed > self.limits.max_read_bytes {
            return Err(BudgetError::ReadBytes);
        }
        self.reserved_read_bytes = remaining;
        self.usage.read_bytes = consumed;
        Ok(())
    }

    /// Releases reserved read bytes that the work did not consume.
    pub fn release_read_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let remaining = self.reserved_read_bytes.checked_sub(bytes).ok_or(BudgetError::Unreserved)?;
        self.reserved_read_bytes = remaining;
        Ok(())
    }

    /// Reserves decoded bytes for work that has not produced them yet.
    pub fn reserve_decoded_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let held = self
            .usage
            .decoded_bytes
            .checked_add(self.reserved_decoded_bytes)
            .and_then(|held| held.checked_add(bytes))
            .ok_or(BudgetError::DecodedBytes)?;
        if held > self.limits.max_decoded_bytes {
            return Err(BudgetError::DecodedBytes);
        }
        self.reserved_decoded_bytes = held - self.usage.decoded_bytes;
        Ok(())
    }

    /// Records the bytes a reserved decode actually produced.
    pub fn settle_decoded_bytes(&mut self, actual: u64) -> Result<(), BudgetError> {
        let remaining = self.reserved_decoded_bytes.checked_sub(actual).ok_or(BudgetError::Unreserved)?;
        let consumed = self.usage.decoded_bytes.checked_add(actual).ok_or(BudgetError::DecodedBytes)?;
        if consumed > self.limits.max_decoded_bytes {
            return Err(BudgetError::DecodedBytes);
        }
        self.reserved_decoded_bytes = remaining;
        self.usage.decoded_bytes = consumed;
        Ok(())
    }

    /// Releases reserved decoded bytes that the work did not produce.
    pub fn release_decoded_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let remaining = self.reserved_decoded_bytes.checked_sub(bytes).ok_or(BudgetError::Unreserved)?;
        self.reserved_decoded_bytes = remaining;
        Ok(())
    }

    /// Reserves bytes that stay live while the work runs.
    ///
    /// Retained bytes are bounded by `max_cache_bytes`. The tracker keeps their
    /// high-water mark so peak retention can be reported apart from cumulative reads
    /// and decodes.
    pub fn reserve_cache_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let next = self.usage.cache_bytes.checked_add(bytes).ok_or(BudgetError::CacheBytes)?;
        if next > self.limits.max_cache_bytes {
            return Err(BudgetError::CacheBytes);
        }
        self.usage.cache_bytes = next;
        self.peak_cache_bytes = self.peak_cache_bytes.max(next);
        Ok(())
    }

    /// Releases retained bytes that are no longer live.
    pub fn release_cache_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let remaining = self.usage.cache_bytes.checked_sub(bytes).ok_or(BudgetError::Unreserved)?;
        self.usage.cache_bytes = remaining;
        Ok(())
    }

    /// Reserves bytes scanned during a signature or window search.
    pub fn reserve_scan_bytes(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let next = self.usage.scan_bytes.checked_add(bytes).ok_or(BudgetError::ScanBytes)?;
        if next > self.limits.max_scan_bytes {
            return Err(BudgetError::ScanBytes);
        }
        self.usage.scan_bytes = next;
        Ok(())
    }

    /// Reserves one container member for the current query.
    pub fn reserve_container_member(&mut self) -> Result<(), BudgetError> {
        let next = self.usage.container_members.checked_add(1).ok_or(BudgetError::ContainerMembers)?;
        if next > self.limits.max_container_members {
            return Err(BudgetError::ContainerMembers);
        }
        self.usage.container_members = next;
        Ok(())
    }

    /// Reserves one reference edge.
    pub fn reserve_reference(&mut self) -> Result<(), BudgetError> {
        let next = self.usage.references.checked_add(1).ok_or(BudgetError::References)?;
        if next > self.limits.max_references {
            return Err(BudgetError::References);
        }
        self.usage.references = next;
        Ok(())
    }

    /// Reserves one layout node.
    pub fn reserve_node(&mut self) -> Result<(), BudgetError> {
        let next = self.usage.nodes.checked_add(1).ok_or(BudgetError::Nodes)?;
        if next > self.limits.max_nodes {
            return Err(BudgetError::Nodes);
        }
        self.usage.nodes = next;
        Ok(())
    }

    /// Reserves one pointer hop.
    pub fn reserve_pointer_hop(&mut self) -> Result<(), BudgetError> {
        let next = self.usage.pointer_hops.checked_add(1).ok_or(BudgetError::PointerHops)?;
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
