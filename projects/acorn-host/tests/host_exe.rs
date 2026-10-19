//! `host_exe`：按平台分发到 `PE32+` / `ELF64` / `Mach-O`。

use acorn_host::host_exe::build_host_executable;

#[test]
fn windows_host_exe_is_pe() {
    let bytes = build_host_executable("windows", None, &[]).expect("pe");
    assert_eq!(&bytes[0..2], b"MZ");
}

#[test]
fn linux_host_exe_is_elf() {
    let bytes = build_host_executable("linux", None, &[]).expect("elf");
    assert_eq!(&bytes[0..4], b"\x7fELF");
}

#[test]
fn macos_host_exe_is_mach_o() {
    let bytes = build_host_executable("macos", None, &[]).expect("mach-o");
    assert_eq!(&bytes[..4], &0xFEED_FACFu32.to_le_bytes());
}

#[test]
fn embeds_native_logic_and_rodata_tail() {
    let bytes = build_host_executable("windows", Some(b"logic"), b"asgard-ui").expect("pe");
    assert!(bytes.windows(5).any(|window| window == b"logic"));
    assert!(bytes.windows(9).any(|window| window == b"asgard-ui"));
}
