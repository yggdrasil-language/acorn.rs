use acorn_core::{Budget, BudgetTracker};
use acorn_docx::OpcPackage;

fn stored_zip(path: &str, payload: &[u8]) -> Vec<u8> {
    let name = path.as_bytes();
    let crc = crc32(payload);

    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&crc.to_le_bytes());
    archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive.extend_from_slice(name);
    archive.extend_from_slice(payload);

    let cd_offset = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    // 42 bytes after signature (APPNOTE total fixed = 46).
    let mut cd_fixed = [0u8; 42];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
    cd_fixed[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
    cd_fixed[38..42].copy_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&cd_fixed);
    archive.extend_from_slice(name);

    let cd_size = archive.len() - cd_offset;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
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

fn minimal_docx_zip() -> Vec<u8> {
    stored_zip("word/document.xml", b"<w:document/>")
}

#[test]
fn opens_zip_and_reads_part_by_path() {
    let pkg = OpcPackage::open("docx.zip", minimal_docx_zip()).expect("open");
    assert!(pkg.part("word/document.xml").is_some());
    let xml = pkg.read_part("word/document.xml", &mut BudgetTracker::new(Budget::default())).expect("read");
    assert_eq!(xml, b"<w:document/>");
}

#[test]
fn normalizes_backslash_part_paths() {
    let pkg = OpcPackage::open("docx.zip", minimal_docx_zip()).expect("open");
    assert!(pkg.part(r"word\document.xml").is_some());
}

#[test]
fn reads_deflated_part_through_opc_package() {
    let payload = b"<w:document/>";
    let compressed = raw_deflate(payload);
    let zip = deflate_zip("word/document.xml", payload, &compressed);
    let pkg = OpcPackage::open("deflate.docx", zip).expect("open");
    let xml = pkg.read_part("word/document.xml", &mut BudgetTracker::new(Budget::default())).expect("read");
    assert_eq!(xml, payload);
}

fn raw_deflate(data: &[u8]) -> Vec<u8> {
    acorn_codec::compression::deflate::Deflate::encode(data, acorn_codec::compression::deflate::DeflateLevel::Default)
}

fn deflate_zip(path: &str, payload: &[u8], compressed: &[u8]) -> Vec<u8> {
    let name = path.as_bytes();
    let crc = crc32(payload);

    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&crc.to_le_bytes());
    archive.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive.extend_from_slice(name);
    archive.extend_from_slice(compressed);

    let cd_offset = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    // 42 bytes after signature (APPNOTE total fixed = 46).
    let mut cd_fixed = [0u8; 42];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[6..8].copy_from_slice(&[0x08, 0x00]);
    cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
    cd_fixed[16..20].copy_from_slice(&(compressed.len() as u32).to_le_bytes());
    cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
    cd_fixed[38..42].copy_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&cd_fixed);
    archive.extend_from_slice(name);

    let cd_size = archive.len() - cd_offset;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

fn stored_zip_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
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
    let cd_size = central.len();
    archive.extend_from_slice(&central);
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

#[test]
fn reads_each_image_part_without_merging_basenames() {
    let document: &[u8] = b"<w:document/>";
    let relationships: &[u8] = b"<Relationships/>";
    let word_image: &[u8] = &[1, 2, 3, 4];
    let root_image: &[u8] = &[9, 9, 9, 9];

    let zip = stored_zip_entries(&[
        ("word/document.xml", document),
        ("word/_rels/document.xml.rels", relationships),
        ("word/media/image1.png", word_image),
        ("media/image1.png", root_image),
    ]);
    let package = OpcPackage::open("picture.docx", zip).expect("open");
    assert_eq!(package.part_paths().len(), 4);

    let mut ledger = BudgetTracker::new(Budget::default());
    assert_eq!(package.read_part("word/media/image1.png", &mut ledger).expect("word image"), word_image);
    assert_eq!(package.read_part("media/image1.png", &mut ledger).expect("root image"), root_image);
    assert_eq!(
        ledger.usage().read_bytes,
        (word_image.len() + root_image.len()) as u64,
        "one ledger covers every part of the document"
    );
}
