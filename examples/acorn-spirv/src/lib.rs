#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

mod bytes;
mod error;
mod header;
mod instruction;
mod module;

/// Byte-slice validation entry and its error surface.
pub use bytes::{SpirvByteError, validate_spirv_bytes};
/// Structural failure classification and its byte evidence.
pub use error::{SpirvEvidence, SpirvLayoutError};
/// Header constants and the recorded id bound.
pub use header::{
    SPIRV_HEADER_BYTES, SPIRV_HEADER_WORDS, SPIRV_ID_BOUND_CEILING, SPIRV_MAGIC, SPIRV_WORD_BYTES, SpirvBoundSuspicion,
    SpirvHeader,
};
/// Instruction layout and the lazy instruction walk.
pub use instruction::{SpirvInstruction, SpirvInstructions};
/// Bounded module view, validation summary and the validation entry.
pub use module::{SpirvModule, SpirvSummary, validate_spirv};
