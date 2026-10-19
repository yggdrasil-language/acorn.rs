//! `dex`：`classes.dex` 写出与多集合合并。

use acorn_dex::dex::{DexImageBuilder, dex_contains_strings, merge_class_sets};
use acorn_jvm::class::{JvmClassFile, JvmMethodDescriptor, JvmTypeDescriptor};

#[test]
fn dex_has_class_defs_and_magic() {
    let mut bridge = JvmClassFile::new("com/asgard/runtime/AsgardHostBridge");
    bridge.access_flags = 0x0031;
    bridge.push_method(
        "invokeExport",
        JvmMethodDescriptor::new(vec![JvmTypeDescriptor::Object("java/lang/String".into())], JvmTypeDescriptor::Void),
    );
    bridge.methods[0].access_flags = 0x0109;
    let bytes = bridge.to_bytes().expect("class");
    let mut builder = DexImageBuilder::new();
    builder.add_class("com/asgard/runtime/AsgardHostBridge", &bytes);
    let dex = builder.build().expect("dex");
    assert!(dex.starts_with(b"dex\n035"));
    assert!(dex.windows(b"AsgardHostBridge".len()).any(|window| window == b"AsgardHostBridge"));
}

#[test]
fn merge_two_minimal_dex() {
    let a_bytes = JvmClassFile::new("com/foo/A").to_bytes().unwrap();
    let b_bytes = JvmClassFile::new("com/bar/B").to_bytes().unwrap();
    let merged = merge_class_sets(&[&[("com/foo/A".to_string(), a_bytes)], &[("com/bar/B".to_string(), b_bytes)]]).unwrap();
    assert!(merged.starts_with(b"dex\n035"));
    assert!(dex_contains_strings(&merged, &["com/foo/A", "com/bar/B"]).unwrap());
}

#[test]
fn rejects_non_dex_input() {
    let error = dex_contains_strings(b"not a dex", &["x"]).expect_err("非 dex 输入必须失败");
    assert!(error.to_string().contains("dex"), "{error}");
}
