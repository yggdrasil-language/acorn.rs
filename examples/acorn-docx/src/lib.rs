#![warn(missing_docs)]
//! DOCX OPC package access for Acorn document pipelines.

mod package;
mod rels;
mod xml;

pub use package::{normalize_part_path, OpcError, OpcPackage};
pub use rels::{parse_relationship_targets, parse_relationships, OpcRelationship};
