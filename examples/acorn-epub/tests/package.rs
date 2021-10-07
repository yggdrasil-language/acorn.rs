use acorn_core::{Budget, BudgetTracker};
use acorn_epub::OcfPackage;

fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut central = Vec::new();

    for (path, payload) in entries {
        let name = path.as_bytes();
        let crc = crc32(payload);
        let local_offset = archive.len();

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&[0x00, 0x00]);
        archive.extend_from_slice(name);
        archive.extend_from_slice(payload);

        central.extend_from_slice(b"PK\x01\x02");
        // 42 bytes after signature (APPNOTE total fixed = 46).
        let mut cd_fixed = [0u8; 42];
        cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
        cd_fixed[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
        cd_fixed[38..42].copy_from_slice(&(local_offset as u32).to_le_bytes());
        central.extend_from_slice(&cd_fixed);
        central.extend_from_slice(name);
    }

    let cd_offset = archive.len();
    archive.extend_from_slice(&central);
    let cd_size = central.len();
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let bit = crc & 1;
            crc >>= 1;
            if bit != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn minimal_epub_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
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
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Chapter</p></body></html>"#,
        ),
    ])
}

#[test]
fn opens_epub_and_reads_opf_metadata() {
    let package = OcfPackage::open("book.epub", minimal_epub_zip()).expect("open epub");
    assert_eq!(package.root_opf_path(), "OEBPS/content.opf");
    let opf = package.read_opf(&mut BudgetTracker::new(Budget::default())).expect("read opf");
    assert_eq!(opf.metadata.title.as_deref(), Some("Sample Book"));
    assert_eq!(opf.metadata.language.as_deref(), Some("en"));
    assert_eq!(opf.spine.len(), 1);
    assert!(opf.manifest.contains_key("ch1"));
}

#[test]
fn resolves_manifest_href_to_member() {
    let package = OcfPackage::open("book.epub", minimal_epub_zip()).expect("open epub");
    let path = OcfPackage::resolve_href(package.root_opf_path(), "chapter.xhtml");
    assert_eq!(path, "OEBPS/chapter.xhtml");
    let bytes = package.read_member(&path, &mut BudgetTracker::new(Budget::default())).expect("read chapter");
    assert!(bytes.windows(7).any(|window| window == b"Chapter"));
}

fn richer_epub_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Navigation Sample</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="text/chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="text/chapter2.xhtml" media-type="application/xhtml+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="cover" href="images/cover.png" media-type="image/png"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>"#;
    let nav = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><body><nav><ol>
  <li><a href="text/chapter1.xhtml">One</a></li>
  <li><a href="text/chapter2.xhtml">Two</a></li>
</ol></nav></body></html>"#;

    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        (
            "OEBPS/text/chapter1.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><body><p>First</p></body></html>"#,
        ),
        (
            "OEBPS/text/chapter2.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Second</p></body></html>"#,
        ),
        ("OEBPS/nav.xhtml", nav),
        ("OEBPS/images/cover.png", &[0x89, b'P', b'N', b'G']),
    ])
}

#[test]
fn reads_spine_navigation_and_image_members() {
    let package = OcfPackage::open("rich.epub", richer_epub_zip()).expect("open epub");
    let mut ledger = BudgetTracker::new(Budget::default());

    let opf = package.read_opf(&mut ledger).expect("read opf");
    assert_eq!(opf.spine.len(), 2);
    assert_eq!(opf.spine[1].idref, "ch2");
    assert_eq!(opf.manifest.get("nav").expect("nav entry").properties.as_deref(), Some("nav"));
    assert_eq!(opf.manifest.get("cover").expect("cover entry").media_type, "image/png");

    let nav_path = OcfPackage::resolve_href(package.root_opf_path(), "nav.xhtml");
    assert_eq!(nav_path, "OEBPS/nav.xhtml");
    assert!(package.read_member(&nav_path, &mut ledger).expect("nav bytes").starts_with(b"<?xml"));

    let cover_path = OcfPackage::resolve_href(package.root_opf_path(), "images/cover.png");
    assert_eq!(cover_path, "OEBPS/images/cover.png");
    assert_eq!(package.read_member(&cover_path, &mut ledger).expect("cover bytes"), [0x89, b'P', b'N', b'G']);
}

#[test]
fn reports_a_missing_member_by_path() {
    let package = OcfPackage::open("rich.epub", richer_epub_zip()).expect("open epub");
    let mut ledger = BudgetTracker::new(Budget::default());
    let error = package.read_member("OEBPS/images/absent.png", &mut ledger).expect_err("absent member");
    assert!(matches!(error, acorn_epub::OcfError::MemberNotFound(_)), "got {error}");
}
