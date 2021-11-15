use acorn_pe::elf::{
    SharedElfDataReference, SharedElfDataRelocation, SharedElfImage, SharedElfImportCall, SharedElfWriter, SharedObjectExport,
    parse_elf64,
};

fn image(text: Vec<u8>) -> SharedElfImage {
    SharedElfImage { text, rodata: Vec::new(), data: Vec::new(), bss: Vec::new() }
}

#[test]
fn writes_et_dyn_aarch64_with_export() {
    let text = vec![0xC0, 0x03, 0x5F, 0xD6];
    let image = image(text);
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
        image(text),
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

    // 首个 `PT_LOAD` 必须覆盖全部动态表，否则宿主 `dlopen` 会读到未映射页。
    let phoff = u64::from_le_bytes(bytes[32..40].try_into().expect("e_phoff")) as usize;
    let p_filesz = u64::from_le_bytes(bytes[phoff + 32..phoff + 40].try_into().expect("p_filesz"));
    assert!(p_filesz > 0, "PT_LOAD 必须覆盖动态表");
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
fn consumer_with_imports() -> (SharedElfImage, Vec<SharedObjectExport>, Vec<String>, Vec<SharedElfImportCall>) {
    let text = vec![0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, 0xC3];
    (
        image(text),
        vec![SharedObjectExport { name: "main".into(), text_offset: 0 }],
        vec!["acorn_probe".into()],
        vec![SharedElfImportCall { rel32_field: 2, import: 0 }],
    )
}

fn write_imports(
    image: &SharedElfImage,
    exports: &[SharedObjectExport],
    imports: &[String],
    calls: &[SharedElfImportCall],
) -> Vec<u8> {
    SharedElfWriter::write_x86_64_with_imports(image, exports, imports, calls, &[], &[]).expect("write")
}

#[test]
fn writes_undefined_imports_with_relocations() {
    let (image, exports, imports, calls) = consumer_with_imports();
    let bytes = write_imports(&image, &exports, &imports, &calls);
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

/// 同一导入的多个调用点必须各自回填，且指向同一个 GOT 槽。
///
/// 旧 API 把导入与唯一的 `rel32` 字段绑在一起，第二个调用点要么重复声明符号、
/// 要么错配槽位。导入符号表与调用点分离后，调用点只携带导入序号。
#[test]
fn multiple_call_sites_share_one_import_slot() {
    // 两个 `call qword ptr [rip+disp32]`，中间填 4 字节以分开字段。
    let mut text = vec![0xFF, 0x15, 0, 0, 0, 0];
    text.extend_from_slice(&[0x90, 0x90, 0x90, 0x90]);
    text.extend_from_slice(&[0xFF, 0x15, 0, 0, 0, 0, 0xC3]);
    let image = image(text);
    let exports = vec![SharedObjectExport { name: "main".into(), text_offset: 0 }];
    let imports = vec!["acorn_probe".into()];
    let calls = vec![SharedElfImportCall { rel32_field: 2, import: 0 }, SharedElfImportCall { rel32_field: 12, import: 0 }];
    let bytes = write_imports(&image, &exports, &imports, &calls);

    // 两个 `disp32` 必须解析到同一个 GOT 槽：字段虚拟地址之差抵消后位移不同，
    // 但「字段地址 + 4 + 位移」必须相等。
    let text_off = 64 + 56 * u16::from_le_bytes([bytes[56], bytes[57]]) as usize;
    let text_off = (text_off + 15) & !15;
    let first = i32::from_le_bytes(bytes[text_off + 2..text_off + 6].try_into().unwrap()) as i64;
    let second = i32::from_le_bytes(bytes[text_off + 12..text_off + 16].try_into().unwrap()) as i64;
    let first_slot = text_off as i64 + 2 + 4 + first;
    let second_slot = text_off as i64 + 12 + 4 + second;
    assert_eq!(first_slot, second_slot, "同一导入的调用点必须共享 GOT 槽");
    assert_eq!(String::from_utf8_lossy(&bytes).matches("acorn_probe").count(), 1, "符号只应声明一次");
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
        &image(vec![0xB8, 0x2A, 0x00, 0x00, 0x00, 0xC3]),
        &[SharedObjectExport { name: "acorn_probe".into(), text_offset: 0 }],
    )
    .expect("write provider");
    let (image, exports, imports, calls) = consumer_with_imports();
    let consumer = write_imports(&image, &exports, &imports, &calls);

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

/// 数据段镜像：`main` 通过 `.data` 里保存的函数指针间接调用 `answer`。
///
/// `.data` 中的指针由 `R_X86_64_RELATIVE` 在装载时写入（镜像基址 + 加数 = 导出
/// 函数虚拟地址），`call [rip+disp32]` 经 `shared_elf_data_reference` 指向数据槽。
fn image_with_pointer_data() -> (SharedElfImage, Vec<SharedObjectExport>, Vec<SharedElfDataRelocation>, Vec<SharedElfDataReference>) {
    // 0: `call qword ptr [rip+disp32]`；2: disp32；6: ret。16: `mov eax, 42; ret`。
    let mut text = vec![0xFF, 0x15, 0, 0, 0, 0, 0xC3];
    while text.len() < 16 {
        text.push(0x90);
    }
    text.extend_from_slice(&[0xB8, 0x2A, 0x00, 0x00, 0x00, 0xC3]);
    let image = SharedElfImage { text, rodata: Vec::new(), data: vec![0; 8], bss: Vec::new() };
    let exports = vec![
        SharedObjectExport { name: "main".into(), text_offset: 0 },
        SharedObjectExport { name: "answer".into(), text_offset: 16 },
    ];
    // 加数 16 是本镜像内 `answer` 的虚拟地址（`p_vaddr == 文件偏移`）。
    let relocations = vec![SharedElfDataRelocation { offset: 0, addend: 16 }];
    let references = vec![SharedElfDataReference { rel32_field: 2, data_offset: 0 }];
    (image, exports, relocations, references)
}

#[test]
fn writes_relative_relocations_for_data() {
    let (image, exports, relocations, references) = image_with_pointer_data();
    let bytes = SharedElfWriter::write_x86_64_with_imports(&image, &exports, &[], &[], &relocations, &references).expect("write");
    let header = parse_elf64(&bytes).expect("parse");
    assert_eq!(header.file_type, 3);
    // 没有任何导入时仍必须输出 `DT_RELA`（数据自相对指针）。
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(haystack.contains("answer"), "导出必须进入动态符号表");

    let phnum = u16::from_le_bytes([bytes[56], bytes[57]]) as usize;
    let mut writable: Vec<(u64, u64)> = Vec::new();
    for index in 0..phnum {
        let base = 64 + index * 56;
        let p_type = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
        let flags = u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap());
        let offset = u64::from_le_bytes(bytes[base + 8..base + 16].try_into().unwrap());
        let vaddr = u64::from_le_bytes(bytes[base + 16..base + 24].try_into().unwrap());
        if p_type == 1 && flags & 0b10 != 0 {
            writable.push((offset, vaddr));
        }
    }
    assert_eq!(writable.len(), 1, "数据段必须独占一个可写 `PT_LOAD`");
    assert_eq!(writable[0].0, writable[0].1, "数据段 `p_vaddr` 必须等于文件偏移");
    assert_eq!(writable[0].0 % 0x1000, 0, "数据段必须页对齐，不能与只读段共享页");
}

/// 真实装载：`R_X86_64_RELATIVE` 必须把数据槽写成导出函数地址，间接调用返回 42。
#[cfg(target_os = "linux")]
#[test]
fn linux_dlopen_applies_relative_relocations() {
    use std::ffi::CString;

    unsafe extern "C" {
        fn dlopen(filename: *const i8, flag: i32) -> *mut core::ffi::c_void;
        fn dlsym(handle: *mut core::ffi::c_void, symbol: *const i8) -> *mut core::ffi::c_void;
        fn dlclose(handle: *mut core::ffi::c_void) -> i32;
        fn dlerror() -> *const i8;
    }

    let (image, exports, relocations, references) = image_with_pointer_data();
    let bytes = SharedElfWriter::write_x86_64_with_imports(&image, &exports, &[], &[], &relocations, &references).expect("write");
    let path = std::env::temp_dir().join(format!("libacorn_relative_{}.so", std::process::id()));
    std::fs::write(&path, &bytes).expect("write .so");
    let path_name = CString::new(path.to_string_lossy().as_bytes()).expect("utf8 path");

    unsafe {
        let handle = dlopen(path_name.as_ptr(), 2);
        assert!(!handle.is_null(), "dlopen: {}", std::ffi::CStr::from_ptr(dlerror()).to_string_lossy());
        let main = dlsym(handle, c"main".as_ptr());
        assert!(!main.is_null(), "dlsym main");
        let main: extern "C" fn() -> i32 = std::mem::transmute(main);
        assert_eq!(main(), 42, "数据槽中的函数指针必须由 RELATIVE 重定位解析");
        dlclose(handle);
    }
    let _ = std::fs::remove_file(&path);
}
