use std::collections::HashMap;

use acorn_core::ParseBudget;

use crate::OpcPackage;
use crate::xml::{attribute_value, document_root, elements_by_local_name, parse_xml_bytes};
use crate::OpcError;

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
}
