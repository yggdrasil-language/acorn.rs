use acorn_pe::pe::{NativeExport, NativePeImage, NativePeWriter};

#[test]
fn writes_amd64_console_executable() {
    let image = NativePeImage {
        text: vec![0x31, 0xC0, 0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
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
fn runs_stub_executable() {
    let image = NativePeImage {
        text: vec![0x31, 0xC0, 0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
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
