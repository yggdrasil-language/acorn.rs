use acorn_pe::elf::{NativeElfImage, NativeElfWriter};

/// `mov eax, 60; mov edi, 42; syscall`：最小 Linux `exit(42)` 进程入口。
const EXIT_42: [u8; 12] = [
    0xB8, 0x3C, 0x00, 0x00, 0x00, // mov eax, 60 (SYS_exit)
    0xBF, 0x2A, 0x00, 0x00, 0x00, // mov edi, 42
    0x0F, 0x05, // syscall
];

#[test]
fn writes_elf_magic_and_entry() {
    let image = NativeElfImage { text: vec![0x31, 0xC0, 0xC3], rodata: Vec::new(), entry_point: 0 };
    let bytes = NativeElfWriter::write_executable(&image).expect("write elf");
    assert_eq!(&bytes[0..4], b"\x7fELF");
    assert_eq!(bytes[4], 2);
    assert_eq!(u16::from_le_bytes([bytes[16], bytes[17]]), 2);
    assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), 62);
}

/// 程序头必须把整个文件身份映射到 `ELF_LOAD_BASE`，入口等于装载基址加节内偏移。
#[test]
fn maps_single_load_segment_and_fixes_entry_vaddr() {
    let image = NativeElfImage { text: EXIT_42.to_vec(), rodata: Vec::new(), entry_point: 0 };
    let bytes = NativeElfWriter::write_executable(&image).expect("write elf");

    // ELF64 头：`e_entry` 在 24、`e_phoff` 在 32、`e_phnum` 在 56。
    let entry = u64::from_le_bytes(bytes[24..32].try_into().expect("e_entry"));
    let phoff_slice = &bytes[32..40];
    let phoff = u64::from_le_bytes(phoff_slice.try_into().expect("e_phoff"));
    assert_eq!(phoff, 64, "程序头紧随 ELF64 头");
    assert_eq!(u16::from_le_bytes([bytes[56], bytes[57]]), 1, "只有一个 PT_LOAD");

    // 程序头：`p_offset` 在 8、`p_vaddr` 在 16、`p_filesz` 在 32、`p_memsz` 在 40。
    let phindex = usize::try_from(phoff).expect("phoff fits usize");
    let p_offset = u64::from_le_bytes(bytes[phindex + 8..phindex + 16].try_into().expect("p_offset"));
    let p_vaddr = u64::from_le_bytes(bytes[phindex + 16..phindex + 24].try_into().expect("p_vaddr"));
    let p_filesz = u64::from_le_bytes(bytes[phindex + 32..phindex + 40].try_into().expect("p_filesz"));
    let p_memsz = u64::from_le_bytes(bytes[phindex + 40..phindex + 48].try_into().expect("p_memsz"));
    assert_eq!(p_offset, 0, "整个文件从偏移 0 装载");
    assert_eq!(p_vaddr, 0x400000, "装载基址");
    assert_eq!(p_filesz, bytes.len() as u64);
    assert_eq!(p_memsz, bytes.len() as u64);

    // `.text` 从程序头之后的对齐偏移开始，条目地址必须是 vaddr + 节内偏移。
    let text_offset: usize = ((64 + 56) + 15) / 16 * 16;
    assert_eq!(entry, 0x400000u64 + text_offset as u64, "入口必须落在身份映射的 `.text` 中");
    assert_eq!(&bytes[text_offset..text_offset + EXIT_42.len()], &EXIT_42);
}

#[test]
fn rejects_entry_point_outside_text() {
    let image = NativeElfImage { text: vec![0xC3], rodata: Vec::new(), entry_point: 1 };
    let error = NativeElfWriter::write_executable(&image).expect_err("越界入口必须被拒绝");
    assert!(matches!(error, acorn_pe::elf::ElfWriteError::EntryOutOfRange { .. }), "{error:?}");
}

/// 真实装载：Linux 内核把镜像跳到入口，进程以入口设置的状态码退出。
#[cfg(target_os = "linux")]
#[test]
fn linux_loads_and_runs_the_executable() {
    use std::os::unix::fs::PermissionsExt;

    let image = NativeElfImage { text: EXIT_42.to_vec(), rodata: Vec::new(), entry_point: 0 };
    let bytes = NativeElfWriter::write_executable(&image).expect("write elf");
    let path = std::env::temp_dir().join(format!("acorn-elf-exit-{}", std::process::id()));
    std::fs::write(&path, &bytes).expect("write elf file");
    let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("chmod");

    let status = std::process::Command::new(&path).status().expect("load and run ELF");
    let _ = std::fs::remove_file(&path);
    assert_eq!(status.code(), Some(42), "入口设置的进程退出码");
}
