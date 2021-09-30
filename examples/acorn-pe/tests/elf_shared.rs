use acorn_pe::elf::{SharedElfImage, SharedElfWriter, SharedObjectExport, parse_elf64};

#[test]
fn writes_et_dyn_aarch64_with_export() {
    let text = vec![0xC0, 0x03, 0x5F, 0xD6];
    let image = SharedElfImage { text, rodata: Vec::new(), bss: Vec::new() };
    let exports = vec![
        SharedObjectExport { name: "asgard_invoke_export".into(), text_offset: 0 },
        SharedObjectExport { name: "awsl_call_on_tap".into(), text_offset: 0 },
    ];
    let bytes = SharedElfWriter::write_aarch64(&image, &exports).expect("write");
    let header = parse_elf64(&bytes).expect("parse");
    assert_eq!(header.file_type, 3);
    assert_eq!(header.machine, 183);
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(haystack.contains("asgard_invoke_export"));
    assert!(haystack.contains("awsl_call_on_tap"));
}
