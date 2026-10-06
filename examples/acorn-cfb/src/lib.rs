#![warn(missing_docs)]
//! Compound File Binary Format (CFB / OLE) reader for Acorn.

mod compound;
mod directory;
mod error;
mod fat;
mod header;

pub use compound::CompoundFile;
pub use error::CfbError;
pub use header::{is_compound_file, COMPOUND_FILE_SIGNATURE};
