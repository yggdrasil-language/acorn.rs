//! `mach_o`：64 位 `Mach-O` 可执行镜像。

use acorn_mach_o::mach_o::MachOImageBuilder;

const MH_MAGIC_64: u32 = 0xFEED_FACF;

#[test]
fn mach_o_magic() {
    let exe = MachOImageBuilder::new().build_executable().unwrap();
    assert_eq!(&exe[..4], &MH_MAGIC_64.to_le_bytes());
}

#[test]
fn embeds_code_and_rodata() {
    let mut builder = MachOImageBuilder::new();
    builder.set_code(&[0xC0, 0x03, 0x5F, 0xD6]);
    builder.add_rodata(b"asgard");
    let exe = builder.build_executable().unwrap();
    assert_eq!(&exe[..4], &MH_MAGIC_64.to_le_bytes());
    assert!(exe.windows(b"asgard".len()).any(|window| window == b"asgard"));
}
