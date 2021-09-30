use acorn_pe::elf::{NativeElfImage, NativeElfWriter};

#[test]
fn writes_elf_magic_and_entry() {
    let image = NativeElfImage { text: vec![0x31, 0xC0, 0xC3], rodata: Vec::new(), entry_point: 0 };
    let bytes = NativeElfWriter::write_executable(&image).expect("write elf");
    assert_eq!(&bytes[0..4], b"\x7fELF");
    assert_eq!(bytes[4], 2);
    assert_eq!(u16::from_le_bytes([bytes[16], bytes[17]]), 2);
    assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), 62);
}
