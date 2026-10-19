//! `aarch64::jni_glue`：JNI 胶水机器码与 `ET_DYN` 共享库镜像。

use acorn_aarch64::aarch64::{emit_jni_glue_module, merge_jni_and_logic, ret_bytes};

#[test]
fn jni_glue_writes_loadable_so() {
    let jni = emit_jni_glue_module().expect("emit");
    assert!(jni.module.text.len() > 4);
    let (image, exports) = merge_jni_and_logic(
        jni,
        ret_bytes().to_vec(),
        vec![acorn_pe::elf::SharedObjectExport { name: "asgard_invoke_export".into(), text_offset: 0 }],
    )
    .expect("merge");
    let so = acorn_pe::elf::SharedElfWriter::write_aarch64(&image, &exports).expect("so");
    assert!(so.starts_with(b"\x7fELF"));
    let names = String::from_utf8_lossy(&so);
    assert!(names.contains("JNI_OnLoad"));
    assert!(names.contains("asgard_invoke_export"));
    assert!(names.contains("asgard_patch_native"));
}
