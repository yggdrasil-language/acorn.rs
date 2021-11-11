//! Module header words and the recorded id bound.

/// Magic word that starts every SPIR-V module.
pub const SPIRV_MAGIC: u32 = 0x0723_0203;
/// Size of the module header in bytes.
pub const SPIRV_HEADER_BYTES: u64 = 20;
/// Size of the module header in 32-bit words.
pub const SPIRV_HEADER_WORDS: u64 = 5;
/// Size of one SPIR-V word in bytes.
pub const SPIRV_WORD_BYTES: u64 = 4;
/// Universal ceiling for a declared id bound.
pub const SPIRV_ID_BOUND_CEILING: u32 = 0x003F_FFFF;

/// The five header words of a SPIR-V module, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpirvHeader {
    /// Magic word, always [`SPIRV_MAGIC`] for a structurally valid module.
    pub magic: u32,
    /// Version word, recorded without interpretation.
    pub version: u32,
    /// Generator tool word, recorded without interpretation.
    pub generator: u32,
    /// Upper bound on the ids the module declares.
    pub bound: u32,
    /// Reserved schema word, zero for a structurally valid module.
    pub schema: u32,
}

impl SpirvHeader {
    /// Reports a recorded `bound` that looks implausible, without rejecting it.
    ///
    /// A zero bound addresses no id at all. Anything above
    /// [`SPIRV_ID_BOUND_CEILING`] exceeds the universal id bound. Both are
    /// recorded as facts so the caller decides how strict to be.
    pub const fn bound_suspicion(&self) -> Option<SpirvBoundSuspicion> {
        if self.bound == 0 {
            Some(SpirvBoundSuspicion::Zero)
        }
        else if self.bound > SPIRV_ID_BOUND_CEILING {
            Some(SpirvBoundSuspicion::AboveCeiling { bound: self.bound, ceiling: SPIRV_ID_BOUND_CEILING })
        }
        else {
            None
        }
    }
}

/// Why a declared id bound looks suspicious.
///
/// This is a recorded observation, not a validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpirvBoundSuspicion {
    /// The declared bound is zero.
    Zero,
    /// The declared bound is above the universal ceiling.
    AboveCeiling {
        /// Bound declared by the module.
        bound: u32,
        /// Ceiling that was exceeded.
        ceiling: u32,
    },
}
