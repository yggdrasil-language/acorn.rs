use oak_xml::ast::{XmlElement, XmlValue};

/// Parses UTF-8 XML bytes through `oak-xml`.
pub fn parse_xml_bytes(xml: &[u8]) -> Result<XmlValue, String> {
    let text = std::str::from_utf8(xml).map_err(|error| format!("xml is not utf-8: {error}"))?;
    oak_xml::parse(text)
}

/// Returns the local name after an optional namespace prefix.
pub fn local_name(name: &str) -> &str {
    name.rsplit_once(':').map(|(_, local)| local).unwrap_or(name)
}

/// Returns whether an element's local tag name matches `expected`.
pub fn element_is(element: &XmlElement, expected: &str) -> bool {
    local_name(&element.name) == expected
}

/// Reads an attribute by local name.
pub fn attribute_value(element: &XmlElement, expected: &str) -> Option<String> {
    element
        .attributes
        .iter()
        .find(|attr| local_name(&attr.name) == expected)
        .map(|attr| attr.value.clone())
}

/// Returns the first direct child element with the given local tag name.
pub fn child_element<'a>(element: &'a XmlElement, expected: &str) -> Option<&'a XmlElement> {
    element
        .children
        .iter()
        .filter_map(XmlValue::as_element)
        .find(|child| element_is(child, expected))
}

/// Collects descendant elements whose local tag name matches `expected`.
pub fn elements_by_local_name<'a>(
    element: &'a XmlElement,
    expected: &str,
) -> Vec<&'a XmlElement> {
    let mut matches = Vec::new();
    collect_elements_by_local_name(element, expected, &mut matches);
    matches
}

fn collect_elements_by_local_name<'a>(
    element: &'a XmlElement,
    expected: &str,
    matches: &mut Vec<&'a XmlElement>,
) {
    if element_is(element, expected) {
        matches.push(element);
    }
    for child in element.children.iter().filter_map(XmlValue::as_element) {
        collect_elements_by_local_name(child, expected, matches);
    }
}

/// Returns trimmed text content for an element.
pub fn element_text(element: &XmlElement) -> String {
    let mut text = String::new();
    collect_element_text(element, &mut text);
    text.trim().to_string()
}

fn collect_element_text(element: &XmlElement, out: &mut String) {
    for child in &element.children {
        match child {
            XmlValue::Text(value) => out.push_str(value),
            XmlValue::CData(value) => out.push_str(value),
            XmlValue::Element(child) => collect_element_text(child, out),
            XmlValue::Fragment(values) => {
                for value in values {
                    if let XmlValue::Text(text) = value {
                        out.push_str(text);
                    } else if let XmlValue::Element(child) = value {
                        collect_element_text(child, out);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Unwraps the document root element from an `oak-xml` value.
pub fn document_root(value: &XmlValue) -> Result<&XmlElement, String> {
    match value {
        XmlValue::Element(element) => Ok(element),
        XmlValue::Fragment(values) => values
            .iter()
            .find_map(XmlValue::as_element)
            .ok_or_else(|| "xml fragment has no root element".into()),
        _ => Err("xml has no root element".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        attribute_value, document_root, element_is, elements_by_local_name, parse_xml_bytes,
    };

    #[test]
    fn parses_epub_container_fixture() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
        let value = parse_xml_bytes(xml).expect("epub container");
        let document = document_root(&value).expect("root element");
        assert!(element_is(document, "container"));
        let rootfiles = elements_by_local_name(document, "rootfile");
        assert_eq!(rootfiles.len(), 1);
        assert_eq!(
            attribute_value(rootfiles[0], "full-path").as_deref(),
            Some("OEBPS/content.opf")
        );
    }
}
