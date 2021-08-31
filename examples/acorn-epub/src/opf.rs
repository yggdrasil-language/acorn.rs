use std::collections::HashMap;

use oak_xml::ast::XmlElement;

use crate::xml::{
    attribute_value, child_element, document_root, element_is, element_text, local_name,
    parse_xml_bytes,
};

/// Dublin Core and package metadata from OPF.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpfMetadata {
    pub title: Option<String>,
    pub language: Option<String>,
    pub creators: Vec<String>,
}

/// One manifest item in OPF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Option<String>,
}

/// One spine `itemref` in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineItem {
    pub idref: String,
}

/// Parsed OPF package document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpfDocument {
    pub path: String,
    pub metadata: OpfMetadata,
    pub manifest: HashMap<String, ManifestItem>,
    pub spine: Vec<SpineItem>,
}

/// Parses an OPF document from XML bytes.
pub fn parse_opf_xml(path: &str, xml: &[u8]) -> Result<OpfDocument, String> {
    let value = parse_xml_bytes(xml)?;
    let package = document_root(&value)?;

    let metadata = child_element(package, "metadata")
        .map(parse_metadata)
        .unwrap_or_default();
    let manifest = child_element(package, "manifest")
        .map(parse_manifest)
        .unwrap_or_default();
    let spine = child_element(package, "spine")
        .map(parse_spine)
        .unwrap_or_default();

    Ok(OpfDocument {
        path: path.to_string(),
        metadata,
        manifest,
        spine,
    })
}

fn parse_metadata(element: &XmlElement) -> OpfMetadata {
    let mut metadata = OpfMetadata::default();
    for child in element.children.iter().filter_map(oak_xml::ast::XmlValue::as_element) {
        match local_name(&child.name) {
            "title" => {
                let value = element_text(child);
                if !value.is_empty() {
                    metadata.title = Some(value);
                }
            }
            "language" => {
                let value = element_text(child);
                if !value.is_empty() {
                    metadata.language = Some(value);
                }
            }
            "creator" => {
                let value = element_text(child);
                if !value.is_empty() {
                    metadata.creators.push(value);
                }
            }
            _ => {}
        }
    }
    metadata
}

fn parse_manifest(element: &XmlElement) -> HashMap<String, ManifestItem> {
    let mut manifest = HashMap::new();
    for child in element.children.iter().filter_map(oak_xml::ast::XmlValue::as_element) {
        if !element_is(child, "item") {
            continue;
        }
        if let Some(item) = parse_manifest_item(child) {
            manifest.insert(item.id.clone(), item);
        }
    }
    manifest
}

fn parse_spine(element: &XmlElement) -> Vec<SpineItem> {
    element
        .children
        .iter()
        .filter_map(oak_xml::ast::XmlValue::as_element)
        .filter(|child| element_is(child, "itemref"))
        .filter_map(|child| attribute_value(child, "idref").map(|idref| SpineItem { idref }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_opf_xml;

    #[test]
    fn parses_dc_metadata_fields() {
        let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Sample Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
        let document = parse_opf_xml("content.opf", opf).expect("parse opf");
        assert_eq!(document.metadata.title.as_deref(), Some("Sample Book"));
        assert_eq!(document.metadata.language.as_deref(), Some("en"));
    }
}

fn parse_manifest_item(element: &XmlElement) -> Option<ManifestItem> {
    let id = attribute_value(element, "id")?;
    let href = attribute_value(element, "href")?;
    let media_type = attribute_value(element, "media-type").unwrap_or_default();
    let properties = attribute_value(element, "properties");
    Some(ManifestItem {
        id,
        href,
        media_type,
        properties,
    })
}
