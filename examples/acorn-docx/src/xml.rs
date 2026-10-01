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

/// Unwraps the document root element from an `oak-xml` value.
pub fn document_root(value: &XmlValue) -> Result<&XmlElement, String> {
    match value {
        XmlValue::Element(element) => Ok(element),
        XmlValue::Fragment(values) => values
            .iter()
            .find_map(XmlValue::as_element)
            .ok_or_else(|| "xml fragment has no root element".to_string()),
        _ => Err("xml has no root element".to_string()),
    }
}
