use acorn_epub::parse_opf_xml;

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
