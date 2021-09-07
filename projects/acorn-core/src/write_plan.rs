use std::collections::BTreeMap;

use crate::address::AddressSpaceId;

/// Stable identifier for one planned output slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanSlotId(pub u64);

/// Stable identifier for one relocation entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelocationId(pub u64);

/// Endianness for a patch point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Endian {
    /// Little-endian encoding.
    Little,
    /// Big-endian encoding.
    Big,
}

/// One planned output interval before bytes are encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanSlot {
    /// Slot identity.
    pub id: PlanSlotId,
    /// Human-readable label.
    pub label: String,
    /// Required alignment within the address space.
    pub alignment: u32,
    /// Planned byte length.
    pub size: u64,
    /// Assigned offset after `finalize`.
    pub offset: Option<u64>,
}

/// Target of a relocation that will be patched later.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RelocationTarget {
    /// Absolute offset within the planned address space.
    Absolute {
        /// Final byte offset.
        offset: u64,
    },
    /// Offset relative to a slot start.
    Slot {
        /// Target slot.
        slot: PlanSlotId,
        /// Offset within the slot.
        offset_within: u64,
    },
}

/// One unresolved or resolved relocation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Relocation {
    /// Relocation identity.
    pub id: RelocationId,
    /// What this relocation points to.
    pub target: RelocationTarget,
    /// Resolved absolute offset when known.
    pub resolved_offset: Option<u64>,
}

/// One in-place patch to apply after layout is known.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PatchPoint {
    /// Relocation whose resolved value is written.
    pub relocation: RelocationId,
    /// Slot containing the patch.
    pub slot: PlanSlotId,
    /// Offset within the slot.
    pub offset_within_slot: u64,
    /// Patch width in bytes.
    pub width: u8,
    /// Encoding endianness.
    pub endian: Endian,
}

/// Generation plan for one output address space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlan {
    /// Output address space.
    pub space: AddressSpaceId,
    slots: Vec<PlanSlot>,
    relocations: Vec<Relocation>,
    patch_points: Vec<PatchPoint>,
    finalized_len: Option<u64>,
}

/// Errors while building or finalizing a layout plan.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// Alignment must be a power of two.
    #[error("alignment must be a power of two, got {alignment}")]
    InvalidAlignment {
        /// Requested alignment.
        alignment: u32,
    },
    /// Slot size must be non-zero.
    #[error("slot size must be greater than zero")]
    EmptySlot,
    /// Referenced slot or relocation does not exist.
    #[error("unknown plan reference")]
    UnknownReference,
    /// Plan was already finalized.
    #[error("layout plan is already finalized")]
    AlreadyFinalized,
    /// Plan must be finalized before lookup.
    #[error("layout plan is not finalized")]
    NotFinalized,
}

impl LayoutPlan {
    /// Creates an empty plan for one address space.
    pub fn new(space: AddressSpaceId) -> Self {
        Self {
            space,
            slots: Vec::new(),
            relocations: Vec::new(),
            patch_points: Vec::new(),
            finalized_len: None,
        }
    }

    /// Adds a slot with required alignment and size.
    pub fn add_slot(
        &mut self,
        label: impl Into<String>,
        alignment: u32,
        size: u64,
    ) -> Result<PlanSlotId, PlanError> {
        if self.finalized_len.is_some() {
            return Err(PlanError::AlreadyFinalized);
        }
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(PlanError::InvalidAlignment { alignment });
        }
        if size == 0 {
            return Err(PlanError::EmptySlot);
        }
        let id = PlanSlotId(self.slots.len() as u64 + 1);
        self.slots.push(PlanSlot {
            id,
            label: label.into(),
            alignment,
            size,
            offset: None,
        });
        Ok(id)
    }

    /// Adds a relocation with an unresolved target.
    pub fn add_relocation(&mut self, target: RelocationTarget) -> Result<RelocationId, PlanError> {
        if self.finalized_len.is_some() {
            return Err(PlanError::AlreadyFinalized);
        }
        let id = RelocationId(self.relocations.len() as u64 + 1);
        self.relocations.push(Relocation {
            id,
            target,
            resolved_offset: None,
        });
        Ok(id)
    }

    /// Records a patch point that will consume a relocation value.
    pub fn add_patch_point(
        &mut self,
        relocation: RelocationId,
        slot: PlanSlotId,
        offset_within_slot: u64,
        width: u8,
        endian: Endian,
    ) -> Result<(), PlanError> {
        if self.finalized_len.is_some() {
            return Err(PlanError::AlreadyFinalized);
        }
        if !self.relocations.iter().any(|entry| entry.id == relocation) {
            return Err(PlanError::UnknownReference);
        }
        if !self.slots.iter().any(|entry| entry.id == slot) {
            return Err(PlanError::UnknownReference);
        }
        if !matches!(width, 1 | 2 | 4 | 8) {
            return Err(PlanError::UnknownReference);
        }
        self.patch_points.push(PatchPoint {
            relocation,
            slot,
            offset_within_slot,
            width,
            endian,
        });
        Ok(())
    }

    /// Assigns slot offsets and resolves slot-relative relocations.
    pub fn finalize(&mut self) -> Result<u64, PlanError> {
        if self.finalized_len.is_some() {
            return Err(PlanError::AlreadyFinalized);
        }
        let mut cursor = 0u64;
        for slot in &mut self.slots {
            let alignment = slot.alignment as u64;
            let remainder = cursor % alignment;
            if remainder != 0 {
                cursor += alignment - remainder;
            }
            slot.offset = Some(cursor);
            cursor += slot.size;
        }
        for relocation in &mut self.relocations {
            relocation.resolved_offset = Some(resolve_relocation_target(
                relocation.target.clone(),
                &self.slots,
            )?);
        }
        self.finalized_len = Some(cursor);
        Ok(cursor)
    }

    /// Returns the finalized output length.
    pub fn finalized_len(&self) -> Result<u64, PlanError> {
        self.finalized_len.ok_or(PlanError::NotFinalized)
    }

    /// Returns all planned slots.
    pub fn slots(&self) -> &[PlanSlot] {
        &self.slots
    }

    /// Returns a slot by id.
    pub fn slot(&self, id: PlanSlotId) -> Option<&PlanSlot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    /// Returns all relocations.
    pub fn relocations(&self) -> &[Relocation] {
        &self.relocations
    }

    /// Returns all patch points.
    pub fn patch_points(&self) -> &[PatchPoint] {
        &self.patch_points
    }

    /// Returns patch points grouped by slot for encoding.
    pub fn patch_points_by_slot(&self) -> BTreeMap<PlanSlotId, Vec<&PatchPoint>> {
        let mut grouped = BTreeMap::new();
        for patch in &self.patch_points {
            grouped.entry(patch.slot).or_insert_with(Vec::new).push(patch);
        }
        grouped
    }
}

fn resolve_relocation_target(
    target: RelocationTarget,
    slots: &[PlanSlot],
) -> Result<u64, PlanError> {
    match target {
        RelocationTarget::Absolute { offset } => Ok(offset),
        RelocationTarget::Slot { slot, offset_within } => {
            let entry = slots.iter().find(|entry| entry.id == slot).ok_or(PlanError::UnknownReference)?;
            let base = entry.offset.ok_or(PlanError::NotFinalized)?;
            Ok(base + offset_within)
        }
    }
}
