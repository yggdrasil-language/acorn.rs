use crate::xml::{
    attribute_value, document_root, elements_by_local_name, parse_xml_bytes,
};

/// One `rootfile` entry from `META-INF/container.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRootFile {
    pub full_path: String,
    pub media_type: String,
}

/// Parses EPUB container XML and returns discovered rootfiles.
pub fn parse_container_xml(xml: &[u8]) -> Result<Vec<ContainerRootFile>, String> {
    let value = parse_xml_bytes(xml)?;
    let document = document_root(&value)?;
    let rootfiles = elements_by_local_name(document, "rootfile")
        .into_iter()
        .filter_map(|element| {
            let full_path = attribute_value(element, "full-path")?;
            let media_type = attribute_value(element, "media-type").unwrap_or_default();
            Some(ContainerRootFile {
                full_path,
                media_type,
            })
        })
        .collect::<Vec<_>>();

    if rootfiles.is_empty() {
        return Err("container.xml has no rootfile".into());
    }
    Ok(rootfiles)
}
