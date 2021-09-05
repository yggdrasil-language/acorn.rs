use std::collections::HashMap;

use acorn_core::ParseBudget;

use crate::package::normalize_part_path;
use crate::OpcPackage;
use crate::xml::{attribute_value, document_root, elements_by_local_name, parse_xml_bytes};
use crate::OpcError;

/// Package root relationships part path.
pub const ROOT_RELS_PATH: &str = "_rels/.rels";

/// Office main document relationship type URI.
pub const OFFICE_DOCUMENT_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

/// One OPC relationship entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcRelationship {
    /// Relationship id (`rId1`, …).
    pub id: String,
    /// Relationship type URI.
    pub relationship_type: String,
    /// Target part path or external URI.
    pub target: String,
    /// `TargetMode` when external.
    pub target_mode: Option<String>,
}

/// Parses OPC `.rels` XML into relationship entries keyed by id.
pub fn parse_relationships(xml: &[u8]) -> Result<Vec<OpcRelationship>, OpcError> {
    let value = parse_xml_bytes(xml).map_err(OpcError::Parse)?;
    let root = document_root(&value).map_err(OpcError::Parse)?;
    let mut relationships = Vec::new();
    for relationship in elements_by_local_name(root, "Relationship") {
        let id = attribute_value(relationship, "Id").ok_or_else(|| {
            OpcError::Parse("relationship is missing Id attribute".into())
        })?;
        let relationship_type = attribute_value(relationship, "Type").ok_or_else(|| {
            OpcError::Parse(format!("relationship {id} is missing Type attribute"))
        })?;
        let target = attribute_value(relationship, "Target").ok_or_else(|| {
            OpcError::Parse(format!("relationship {id} is missing Target attribute"))
        })?;
        relationships.push(OpcRelationship {
            id,
            relationship_type,
            target,
            target_mode: attribute_value(relationship, "TargetMode"),
        });
    }
    Ok(relationships)
}

/// Parses OPC `.rels` XML into a target map keyed by relationship id.
pub fn parse_relationship_targets(xml: &[u8]) -> Result<HashMap<String, String>, OpcError> {
    let mut targets = HashMap::new();
    for relationship in parse_relationships(xml)? {
        targets.insert(relationship.id, relationship.target);
    }
    Ok(targets)
}

/// Resolves an OPC relationship target relative to the owning part path.
pub fn resolve_opc_target(source_part: &str, target: &str) -> String {
    if target.starts_with('/') {
        return normalize_part_path(target);
    }
    let source = normalize_part_path(source_part);
    let base_dir = source
        .rfind('/')
        .map(|index| source[..index].to_string())
        .unwrap_or_default();
    let combined = if base_dir.is_empty() {
        target.to_string()
    } else {
        format!("{base_dir}/{target}")
    };
    normalize_part_path(&combined)
}

impl OpcPackage {
    /// Reads and parses an OPC relationships part (for example `word/_rels/document.xml.rels`).
    pub fn read_relationships(
        &self,
        path: &str,
        budget: &ParseBudget,
    ) -> Result<Vec<OpcRelationship>, OpcError> {
        let xml = self.read_part(path, budget)?;
        parse_relationships(&xml)
    }

    /// Reads relationship targets keyed by id, returning an empty map when the part is absent.
    pub fn read_relationship_targets(
        &self,
        path: &str,
        budget: &ParseBudget,
    ) -> Result<HashMap<String, String>, OpcError> {
        match self.read_part(path, budget) {
            Ok(xml) => parse_relationship_targets(&xml),
            Err(OpcError::PartNotFound(_)) => Ok(HashMap::new()),
            Err(error) => Err(error),
        }
    }

    /// Reads package root relationships from `_rels/.rels`.
    pub fn read_root_relationships(
        &self,
        budget: &ParseBudget,
    ) -> Result<Vec<OpcRelationship>, OpcError> {
        match self.read_relationships(ROOT_RELS_PATH, budget) {
            Ok(relationships) => Ok(relationships),
            Err(OpcError::PartNotFound(_)) => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// Resolves the main WordprocessingML document part path from root relationships.
    pub fn main_document_part_path(
        &self,
        budget: &ParseBudget,
    ) -> Result<Option<String>, OpcError> {
        for relationship in self.read_root_relationships(budget)? {
            if relationship.relationship_type == OFFICE_DOCUMENT_RELATIONSHIP_TYPE {
                return Ok(Some(resolve_opc_target("", &relationship.target)));
            }
        }
        Ok(None)
    }
}
