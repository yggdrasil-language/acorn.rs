#![warn(missing_docs)]
//! Random-access byte sources for Acorn.
//!
//! `ByteSource` never pads past EOF or fabricates missing ranges. Short reads
//! and unavailable intervals are reported explicitly.

mod capabilities;
mod identity;
mod memory;
mod memory_sink;
mod partial_source;
mod read;
mod sink;
mod sink_capabilities;
mod source;
mod write;
mod window;

pub use capabilities::SourceCapabilities;
pub use identity::SourceIdentity;
pub use memory::MemorySource;
pub use memory_sink::MemorySink;
pub use partial_source::RangeMapSource;
pub use read::{RangeError, ReadOutcome, ReadResult, SourceRange};
pub use sink::{ByteSink, WriteError};
pub use sink_capabilities::SinkCapabilities;
pub use source::{ByteSource, ReadError};
pub use write::WriteOutcome;
pub use window::ByteWindow;
