use acorn_pe::elf::{SharedElfImage, SharedElfImport, SharedElfWriter, SharedObjectExport, parse_elf64};

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

    // 首个 `PT_LOAD` 必须覆盖整份文件，否则动态表（hash/dynsym/dynstr/dynamic）
    // 落在未映射页，宿主 `dlopen` 会读到段外。
    let phoff = u64::from_le_bytes(bytes[32..40].try_into().expect("e_phoff")) as usize;
    let p_filesz = u64::from_le_bytes(bytes[phoff + 32..phoff + 40].try_into().expect("p_filesz"));
    assert_eq!(p_filesz, bytes.len() as u64, "PT_LOAD 必须覆盖整份文件");
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

/// 带宿主导入的消费者镜像：`main = call [rip+got]; ret`。
fn consumer_with_import() -> (SharedElfImage, Vec<SharedObjectExport>, Vec<SharedElfImport>) {
    let text = vec![0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, 0xC3];
    (
        SharedElfImage { text, rodata: Vec::new(), bss: Vec::new() },
        vec![SharedObjectExport { name: "main".into(), text_offset: 0 }],
        vec![SharedElfImport { symbol: "acorn_probe".into(), rel32_field: 2 }],
    )
}

#[test]
fn writes_undefined_imports_with_relocations() {
    let (image, exports, imports) = consumer_with_import();
    let bytes = SharedElfWriter::write_x86_64_with_imports(&image, &exports, &imports).expect("write");
    let header = parse_elf64(&bytes).expect("parse");
    assert_eq!(header.file_type, 3);
    assert_eq!(header.machine, 62);
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(haystack.contains("acorn_probe"), "导入符号必须进入动态符号表");
    // 导入必须产生独立的可写 GOT 段（PF_W）。
    let phnum = u16::from_le_bytes([bytes[56], bytes[57]]) as usize;
    let mut has_writable = false;
    for index in 0..phnum {
        let base = 64 + index * 56;
        let flags = u32::from_le_bytes(bytes[base + 4..base + 8].try_into().expect("p_flags"));
        if flags & 0b10 != 0 {
            has_writable = true;
        }
    }
    assert!(has_writable, "GOT 必须位于可写段");
}

/// 真实装载：provider 先以 RTLD_GLOBAL 载入，consumer 的未定义符号经 GOT 解析。
#[cfg(target_os = "linux")]
#[test]
fn linux_dlopen_resolves_a_host_import_through_the_got() {
    use std::ffi::CString;

    unsafe extern "C" {
        fn dlopen(filename: *const i8, flag: i32) -> *mut core::ffi::c_void;
        fn dlsym(handle: *mut core::ffi::c_void, symbol: *const i8) -> *mut core::ffi::c_void;
        fn dlclose(handle: *mut core::ffi::c_void) -> i32;
        fn dlerror() -> *const i8;
    }

    let provider = SharedElfWriter::write_x86_64(
        &SharedElfImage { text: vec![0xB8, 0x2A, 0x00, 0x00, 0x00, 0xC3], rodata: Vec::new(), bss: Vec::new() },
        &[SharedObjectExport { name: "acorn_probe".into(), text_offset: 0 }],
    )
    .expect("write provider");
    let (image, exports, imports) = consumer_with_import();
    let consumer = SharedElfWriter::write_x86_64_with_imports(&image, &exports, &imports).expect("write consumer");

    let dir = std::env::temp_dir().join(format!("acorn-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let provider_path = dir.join("libacorn_provider.so");
    let consumer_path = dir.join("libacorn_consumer.so");
    std::fs::write(&provider_path, &provider).expect("write provider");
    std::fs::write(&consumer_path, &consumer).expect("write consumer");
    let provider_name = CString::new(provider_path.to_string_lossy().as_bytes()).expect("utf8 path");
    let consumer_name = CString::new(consumer_path.to_string_lossy().as_bytes()).expect("utf8 path");

    unsafe {
        // RTLD_NOW | RTLD_GLOBAL：provider 的符号进入全局作用域。
        let provider = dlopen(provider_name.as_ptr(), 2 | 0x100);
        assert!(!provider.is_null(), "provider dlopen: {}", std::ffi::CStr::from_ptr(dlerror()).to_string_lossy());
        let consumer = dlopen(consumer_name.as_ptr(), 2);
        assert!(
            !consumer.is_null(),
            "consumer dlopen: {}",
            std::ffi::CStr::from_ptr(dlerror()).to_string_lossy()
        );
        let main = dlsym(consumer, c"main".as_ptr());
        assert!(!main.is_null(), "dlsym main");
        let main: extern "C" fn() -> i32 = std::mem::transmute(main);
        assert_eq!(main(), 42, "导入调用必须到达 provider 导出函数");
        dlclose(consumer);
        dlclose(provider);
    }
    let _ = std::fs::remove_file(&provider_path);
    let _ = std::fs::remove_file(&consumer_path);
    let _ = std::fs::remove_dir(&dir);
}
