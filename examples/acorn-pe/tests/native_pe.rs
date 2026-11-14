use acorn_pe::pe::{NativeExport, NativePeImage, NativePeWriter};

#[test]
fn writes_amd64_console_executable() {
    let image = NativePeImage {
        text: vec![0x31, 0xC0, 0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        text_rel32_fixups: Vec::new(),
            is_dll: false,
        entry_point: 0,
    };
    let bytes = NativePeWriter::write_executable(&image).expect("write pe");
    assert_eq!(&bytes[0..2], b"MZ");
    assert_eq!(&bytes[0x40..0x44], b"PE\0\0");
    assert_eq!(u16::from_le_bytes([bytes[0x44], bytes[0x45]]), 0x8664);

    let pe_offset = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
    let magic = u16::from_le_bytes(bytes[pe_offset + 24..pe_offset + 26].try_into().unwrap());
    assert_eq!(magic, 0x020B);
    let section_count = u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap());
    assert_eq!(section_count, 1);
    let import_rva = u32::from_le_bytes(bytes[pe_offset + 24 + 120..pe_offset + 24 + 124].try_into().unwrap());
    let import_size = u32::from_le_bytes(bytes[pe_offset + 24 + 124..pe_offset + 24 + 128].try_into().unwrap());
    assert_eq!(import_rva, 0);
    assert_eq!(import_size, 0);
}

#[test]
fn writes_dll_with_export_directory() {
    // `mov rax, 42; ret`
    let text = vec![0x48, 0xB8, 42, 0, 0, 0, 0, 0, 0, 0, 0xC3];
    let image = NativePeImage {
        text,
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: vec![NativeExport { name: "main".to_string(), offset: 0 }],
        text_rel32_fixups: Vec::new(),
            is_dll: true,
        entry_point: 0,
    };
    let bytes = NativePeWriter::write_executable(&image).expect("write dll");
    let pe_offset = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
    let characteristics = u16::from_le_bytes(bytes[pe_offset + 22..pe_offset + 24].try_into().unwrap());
    assert_eq!(characteristics & 0x2000, 0x2000, "DLL flag must be set");
    let section_count = u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap());
    assert_eq!(section_count, 2, "text + edata");
    let export_rva = u32::from_le_bytes(bytes[pe_offset + 24 + 112..pe_offset + 24 + 116].try_into().unwrap());
    let export_size = u32::from_le_bytes(bytes[pe_offset + 24 + 116..pe_offset + 24 + 120].try_into().unwrap());
    assert_ne!(export_rva, 0, "export directory must be present");
    assert_eq!(export_size, 40, "export directory size");
    assert!(bytes.windows(4).any(|window| window == b"main"), "export name must be written");
}

#[test]
#[cfg(windows)]
fn loads_exported_function_from_dll_image() {
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut core::ffi::c_void;
        fn GetProcAddress(module: *mut core::ffi::c_void, name: *const u8) -> *mut core::ffi::c_void;
        fn FreeLibrary(module: *mut core::ffi::c_void) -> i32;
    }

    // `mov rax, 42; ret`
    let text = vec![0x48, 0xB8, 42, 0, 0, 0, 0, 0, 0, 0, 0xC3];
    let image = NativePeImage {
        text,
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: vec![NativeExport { name: "main".to_string(), offset: 0 }],
        text_rel32_fixups: Vec::new(),
            is_dll: true,
        entry_point: 0,
    };
    let bytes = NativePeWriter::write_executable(&image).expect("write dll");
    let path = std::env::temp_dir().join(format!("acorn-native-export-{}.dll", std::process::id()));
    std::fs::write(&path, &bytes).expect("write dll file");

    let mut path_bytes = path.to_string_lossy().into_owned().into_bytes();
    path_bytes.push(0);
    unsafe {
        let module = LoadLibraryA(path_bytes.as_ptr());
        assert!(!module.is_null(), "LoadLibraryA failed for {}", path.display());
        let symbol = GetProcAddress(module, b"main\0".as_ptr());
        assert!(!symbol.is_null(), "GetProcAddress(main) failed");
        let main: extern "system" fn() -> i64 = std::mem::transmute(symbol);
        assert_eq!(main(), 42);
        FreeLibrary(module);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg(windows)]
fn resolves_imported_function_through_iat() {
    use acorn_pe::pe::build_native_import_table;

    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut core::ffi::c_void;
        fn GetProcAddress(module: *mut core::ffi::c_void, name: *const u8) -> *mut core::ffi::c_void;
        fn FreeLibrary(module: *mut core::ffi::c_void) -> i32;
    }

    // provider：导出 `nyar_probe`，`mov rax, 7; ret`。
    let provider = NativePeImage {
        text: vec![0x48, 0xB8, 7, 0, 0, 0, 0, 0, 0, 0, 0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: vec![NativeExport { name: "nyar_probe".to_string(), offset: 0 }],
        text_rel32_fixups: Vec::new(),
        is_dll: true,
        entry_point: 0,
    };
    let provider_bytes = NativePeWriter::write_executable(&provider).expect("write provider");

    // consumer：`call [rip+disp32]`（FF 15 disp32）+ `ret`，汇入导入表的 IAT 槽。
    let table = build_native_import_table(&[("nyar_import_provider".to_string(), "nyar_probe".to_string())]).expect("import table");
    let slot = *table.slot_offsets.get(&("nyar_import_provider".to_string(), "nyar_probe".to_string())).expect("slot");
    let consumer = NativePeImage {
        text: vec![0xFF, 0x15, 0, 0, 0, 0, 0xC3],
        rdata: Vec::new(),
        idata: table.bytes,
        imports: table.imports,
        exports: vec![NativeExport { name: "main".to_string(), offset: 0 }],
        text_rel32_fixups: vec![(2, slot)],
        is_dll: true,
        entry_point: 0,
    };
    let consumer_bytes = NativePeWriter::write_executable(&consumer).expect("write consumer");

    let dir = std::env::temp_dir().join(format!("nyar-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("import temp dir");
    // 导入表按模块名解析（`nyar_import_provider.dll`），文件名必须一致。
    let provider_path = dir.join("nyar_import_provider.dll");
    let consumer_path = dir.join("nyar_import_consumer.dll");
    std::fs::write(&provider_path, &provider_bytes).expect("write provider file");
    std::fs::write(&consumer_path, &consumer_bytes).expect("write consumer file");
    let mut provider_name = provider_path.to_string_lossy().into_owned().into_bytes();
    provider_name.push(0);
    let mut consumer_name = consumer_path.to_string_lossy().into_owned().into_bytes();
    consumer_name.push(0);

    unsafe {
        let provider_module = LoadLibraryA(provider_name.as_ptr());
        assert!(!provider_module.is_null(), "provider load failed");
        let consumer_module = LoadLibraryA(consumer_name.as_ptr());
        assert!(!consumer_module.is_null(), "consumer load failed (import table must resolve)");
        let symbol = GetProcAddress(consumer_module, b"main\0".as_ptr());
        assert!(!symbol.is_null(), "consumer export missing");
        let main: extern "system" fn() -> i64 = std::mem::transmute(symbol);
        assert_eq!(main(), 7, "call through IAT must reach the imported function");
        FreeLibrary(consumer_module);
        FreeLibrary(provider_module);
    }
    let _ = std::fs::remove_file(&consumer_path);
    let _ = std::fs::remove_file(&provider_path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
#[cfg(windows)]
fn runs_stub_executable() {
    let image = NativePeImage {
        text: vec![0x31, 0xC0, 0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        text_rel32_fixups: Vec::new(),
            is_dll: false,
        entry_point: 0,
    };
    let bytes = NativePeWriter::write_executable(&image).expect("write pe");
    let path = std::env::temp_dir().join(format!("valkyrie-native-stub-{}.exe", std::process::id()));
    std::fs::write(&path, &bytes).expect("write");
    let status = std::process::Command::new(&path).status().expect("run");
    let _ = std::fs::remove_file(&path);
    assert!(status.success(), "status={status:?}");
}
