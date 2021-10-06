//! `native_pe`：编码结果编排为 `PE32+` 可执行镜像。

use acorn_x64::{
    native_pe::NativeImageBuilder,
    x86_64::{Reg64, X64Instruction},
};

fn pe_offset(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize
}

fn read_section_names(bytes: &[u8]) -> Vec<String> {
    let pe_offset = pe_offset(bytes);
    let section_count = u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap()) as usize;
    let mut names = Vec::new();
    for index in 0..section_count {
        let start = pe_offset + 24 + 240 + index * 40;
        let raw = bytes[start..start + 8].to_vec();
        names.push(String::from_utf8_lossy(&raw).trim_end_matches('\0').to_string());
    }
    names
}

#[test]
fn writes_entry_point_stub() {
    let mut builder = NativeImageBuilder::new();
    builder.push(X64Instruction::Label("_start".to_string()));
    builder.push(X64Instruction::XorReg { dst: Reg64::Rax, src: Reg64::Rax });
    builder.push(X64Instruction::Ret);
    let bytes = builder.build_executable("_start").expect("build pe");

    assert_eq!(&bytes[0..2], b"MZ");
    assert_eq!(&bytes[0x40..0x44], b"PE\0\0");
    let pe_offset = pe_offset(&bytes);
    let entry_rva = u32::from_le_bytes(bytes[pe_offset + 24 + 16..pe_offset + 24 + 20].try_into().unwrap());
    let base_of_code = u32::from_le_bytes(bytes[pe_offset + 24 + 20..pe_offset + 24 + 24].try_into().unwrap());
    assert_eq!(entry_rva, base_of_code, "入口点必须落在 `.text` 起始");
    assert_eq!(read_section_names(&bytes), vec![".text".to_string()]);
}

#[test]
fn writes_import_directory_section() {
    let mut builder = NativeImageBuilder::new();
    let slot = builder.import("KERNEL32.dll", "ExitProcess");
    builder.push(X64Instruction::Label("_start".to_string()));
    builder.push(X64Instruction::CallImport { slot });
    builder.push(X64Instruction::Ret);
    builder.add_rdata("message", b"hello");
    let bytes = builder.build_executable("_start").expect("build pe");

    assert_eq!(read_section_names(&bytes), vec![".text".to_string(), ".rdata".to_string(), ".idata".to_string()]);
    let pe_offset = pe_offset(&bytes);
    let import_rva = u32::from_le_bytes(bytes[pe_offset + 24 + 120..pe_offset + 24 + 124].try_into().unwrap());
    let import_size = u32::from_le_bytes(bytes[pe_offset + 24 + 124..pe_offset + 24 + 128].try_into().unwrap());
    assert_ne!(import_rva, 0);
    // 一个 DLL 的导入目录含终止描述符，共 2 * 20 字节。
    assert_eq!(import_size, 40);
    assert!(bytes.windows(b"kernel32.dll".len()).any(|window| window == b"kernel32.dll"));
    assert!(bytes.windows(b"ExitProcess".len()).any(|window| window == b"ExitProcess"));
}

#[test]
fn rejects_unknown_entry_label() {
    let builder = NativeImageBuilder::new();
    let error = builder.build_executable("missing").expect_err("缺少入口标签必须失败");
    assert!(error.to_string().contains("missing"), "{error}");
}
