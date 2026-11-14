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

/// `mov eax, 42; ret` 与 `mov eax, 7; ret` 两个导出函数。
fn x86_64_exports() -> (SharedElfImage, Vec<SharedObjectExport>) {
    let mut text = vec![0xB8, 0x2A, 0x00, 0x00, 0x00, 0xC3];
    while text.len() < 8 {
        text.push(0);
    }
    text.extend_from_slice(&[0xB8, 0x07, 0x00, 0x00, 0x00, 0xC3]);
    (
        SharedElfImage { text, rodata: Vec::new(), bss: Vec::new() },
        vec![
            SharedObjectExport { name: "answer".into(), text_offset: 0 },
            SharedObjectExport { name: "seven".into(), text_offset: 8 },
        ],
    )
}

#[test]
fn writes_et_dyn_x86_64_with_every_export() {
    let (image, exports) = x86_64_exports();
    let bytes = SharedElfWriter::write_x86_64(&image, &exports).expect("write");
    let header = parse_elf64(&bytes).expect("parse");
    assert_eq!(header.file_type, 3, "ET_DYN");
    assert_eq!(header.machine, 62, "EM_X86_64");
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(haystack.contains("answer"));
    assert!(haystack.contains("seven"));
}

/// 真实装载：Linux `dlopen` 后 `dlsym` 必须解析**每一个**导出符号。
///
/// 单桶 SysV 哈希表若漏写链尾，`dlsym` 只能找到第一个导出；本用例用两个导出
/// 锁住该行为。
#[cfg(target_os = "linux")]
#[test]
fn linux_dlopen_resolves_every_export() {
    use std::ffi::CString;

    unsafe extern "C" {
        fn dlopen(filename: *const i8, flag: i32) -> *mut core::ffi::c_void;
        fn dlsym(handle: *mut core::ffi::c_void, symbol: *const i8) -> *mut core::ffi::c_void;
        fn dlclose(handle: *mut core::ffi::c_void) -> i32;
    }

    let (image, exports) = x86_64_exports();
    let bytes = SharedElfWriter::write_x86_64(&image, &exports).expect("write");
    let path = std::env::temp_dir().join(format!("libacorn_shared_{}.so", std::process::id()));
    std::fs::write(&path, &bytes).expect("write .so");
    let path_c = CString::new(path.to_string_lossy().as_bytes()).expect("utf8 path");

    unsafe {
        let handle = dlopen(path_c.as_ptr(), 2 /* RTLD_NOW */);
        assert!(!handle.is_null(), "dlopen must load the emitted ET_DYN");
        let answer = dlsym(handle, c"answer".as_ptr());
        assert!(!answer.is_null(), "dlsym must resolve `answer`");
        let seven = dlsym(handle, c"seven".as_ptr());
        assert!(!seven.is_null(), "dlsym must resolve `seven`");
        let answer: extern "C" fn() -> i32 = std::mem::transmute(answer);
        let seven: extern "C" fn() -> i32 = std::mem::transmute(seven);
        assert_eq!(answer(), 42);
        assert_eq!(seven(), 7);
        dlclose(handle);
    }
    let _ = std::fs::remove_file(&path);
}
