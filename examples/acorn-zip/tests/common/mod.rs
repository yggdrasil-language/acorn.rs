pub fn minimal_single_file_zip() -> Vec<u8> {
    let mut archive = Vec::new();
    archive.extend_from_slice(b"PK\x03\x04");
    archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0x83, 0x16, 0xdc, 0x8c]);
    archive.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    archive.push(b'a');
    archive.push(b'x');

    let local_size = archive.len();
    archive.extend_from_slice(b"PK\x01\x02");
    let mut cd_fixed = [0u8; 46];
    cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
    cd_fixed[12..16].copy_from_slice(&[0x83, 0x16, 0xdc, 0x8c]);
    cd_fixed[16..20].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    cd_fixed[20..24].copy_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    cd_fixed[24..26].copy_from_slice(&[0x01, 0x00]);
    cd_fixed[38..42].copy_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&cd_fixed);
    archive.push(b'a');

    let cd_size = archive.len() - local_size;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(local_size as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}
