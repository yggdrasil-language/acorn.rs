#![warn(missing_docs)]
//! Random-access byte sources for Acorn.
//!
//! `ByteSource` never pads past EOF or fabricates missing ranges. Short reads
//! and unavailable intervals are reported explicitly.

mod capabilities;
mod identity;
mod memory;
mod partial_source;
mod read;
mod source;
mod window;

pub use capabilities::SourceCapabilities;
pub use identity::SourceIdentity;
pub use memory::MemorySource;
pub use partial_source::RangeMapSource;
pub use read::{RangeError, ReadOutcome, ReadResult, SourceRange};
pub use source::{ByteSource, ReadError};
pub use window::ByteWindow;
