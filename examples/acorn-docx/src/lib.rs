#![warn(missing_docs)]
//! DOCX OPC package access for Acorn document pipelines.

mod package;
mod rels;
mod xml;

pub use package::{OpcError, OpcPackage, normalize_part_path};
pub use rels::{
    OFFICE_DOCUMENT_RELATIONSHIP_TYPE, OpcRelationship, ROOT_RELS_PATH, parse_relationship_targets, parse_relationships,
    resolve_opc_target,
};
