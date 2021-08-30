use acorn_core::ParseBudget;
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
    let mut cd_fixed = [0u8; 46];
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
    let xml = pkg
        .read_part("word/document.xml", &ParseBudget::default())
        .expect("read");
    assert_eq!(xml, b"<w:document/>");
}

#[test]
fn normalizes_backslash_part_paths() {
    let pkg = OpcPackage::open("docx.zip", minimal_docx_zip()).expect("open");
    assert!(pkg.part(r"word\document.xml").is_some());
}
