use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;

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
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut metadata = OpfMetadata::default();
    let mut manifest = HashMap::new();
    let mut spine = Vec::new();

    let mut in_metadata = false;
    let mut in_manifest = false;
    let mut in_spine = false;
    let mut current_dc = None;

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(tag) => handle_tag(
                &tag,
                true,
                &mut in_metadata,
                &mut in_manifest,
                &mut in_spine,
                &mut current_dc,
                &mut metadata,
                &mut manifest,
                &mut spine,
            ),
            Event::Empty(tag) => handle_tag(
                &tag,
                false,
                &mut in_metadata,
                &mut in_manifest,
                &mut in_spine,
                &mut current_dc,
                &mut metadata,
                &mut manifest,
                &mut spine,
            ),
            Event::Text(text) if in_metadata && current_dc.is_some() => {
                if let Ok(decoded) = text.unescape() {
                    let value = decoded.trim().to_string();
                    match current_dc.as_deref() {
                        Some(b"title") if !value.is_empty() => metadata.title = Some(value),
                        Some(b"language") if !value.is_empty() => metadata.language = Some(value),
                        Some(b"creator") if !value.is_empty() => metadata.creators.push(value),
                        _ => {}
                    }
                }
            }
            Event::End(tag) => {
                let local = tag.local_name();
                match local.as_ref() {
                    b"metadata" => {
                        in_metadata = false;
                        current_dc = None;
                    }
                    b"manifest" => in_manifest = false,
                    b"spine" => in_spine = false,
                    b"title" | b"language" | b"creator" => current_dc = None,
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(OpfDocument {
        path: path.to_string(),
        metadata,
        manifest,
        spine,
    })
}

fn handle_tag(
    tag: &quick_xml::events::BytesStart,
    allow_dc_text: bool,
    in_metadata: &mut bool,
    in_manifest: &mut bool,
    in_spine: &mut bool,
    current_dc: &mut Option<Vec<u8>>,
    metadata: &mut OpfMetadata,
    manifest: &mut HashMap<String, ManifestItem>,
    spine: &mut Vec<SpineItem>,
) {
    let local = tag.local_name();
    match local.as_ref() {
        b"metadata" if allow_dc_text => *in_metadata = true,
        b"manifest" if allow_dc_text => *in_manifest = true,
        b"spine" if allow_dc_text => *in_spine = true,
        b"item" if *in_manifest => {
            if let Some(item) = parse_manifest_item(tag) {
                manifest.insert(item.id.clone(), item);
            }
        }
        b"itemref" if *in_spine => {
            if let Some(idref) = attribute_value(tag, b"idref") {
                spine.push(SpineItem { idref });
            }
        }
        b"title" | b"language" | b"creator" if *in_metadata && allow_dc_text => {
            *current_dc = Some(local.as_ref().to_vec());
        }
        _ => {}
    }
}

fn parse_manifest_item(tag: &quick_xml::events::BytesStart) -> Option<ManifestItem> {
    let id = attribute_value(tag, b"id")?;
    let href = attribute_value(tag, b"href")?;
    let media_type = attribute_value(tag, b"media-type").unwrap_or_default();
    Some(ManifestItem {
        id,
        href,
        media_type,
    })
}

fn attribute_value(tag: &quick_xml::events::BytesStart, local: &[u8]) -> Option<String> {
    tag.attributes()
        .filter_map(|attr| attr.ok())
        .find(|attr| attr.key.local_name().as_ref() == local)
        .and_then(|attr| attr.unescape_value().ok())
        .map(|value| value.into_owned())
}
