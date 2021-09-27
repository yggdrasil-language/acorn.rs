use acorn_jvm::jar::JvmJarPackage;

#[test]
fn stored_jar_roundtrips_through_acorn_zip() {
    let mut package = JvmJarPackage::new("app.jar");
    package.main_class = Some("demo.Main".into());
    package.push_entry("demo/Main.class", b"\xca\xfe\xba\xbe".to_vec());
    let bytes = package.to_bytes().expect("encode");
    let decoded = JvmJarPackage::from_bytes("app.jar", &bytes).expect("decode");
    assert_eq!(decoded.main_class.as_deref(), Some("demo.Main"));
    assert_eq!(decoded.entries.len(), 1);
    assert_eq!(decoded.entries[0].path, "demo/Main.class");
    assert_eq!(decoded.entries[0].bytes, b"\xca\xfe\xba\xbe");
}
