use acorn_epub::{attribute_value, document_root, element_is, elements_by_local_name, parse_xml_bytes};

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
    assert_eq!(attribute_value(rootfiles[0], "full-path").as_deref(), Some("OEBPS/content.opf"));
}
