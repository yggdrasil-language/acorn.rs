#![warn(missing_docs)]
//! DOCX OPC package access for Acorn document pipelines.

mod package;

pub use package::{normalize_part_path, OpcError, OpcPackage};
