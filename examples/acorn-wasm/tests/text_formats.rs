//! WAT / WIT 文本写出用例：独立期望文本。

use acorn_wasm::text::{WatDocument, WitPackage, format_wat_document, format_wit_package};

#[test]
fn empty_module_renders_as_single_form() {
    assert_eq!(format_wat_document(&WatDocument::new()), "(module)");
}

#[test]
fn named_module_lists_fields_on_indented_lines() {
    let mut document = WatDocument::new();
    document.module_name = Some("$app".to_string());
    document.push_field("(func $main)");
    document.push_field("  (export \"main\" (func $main))  ");
    let expected = "(module $app\n  (func $main)\n  (export \"main\" (func $main))\n)";
    assert_eq!(format_wat_document(&document), expected);
}

#[test]
fn package_without_interfaces_ends_after_declaration() {
    assert_eq!(format_wit_package(&WitPackage::new("acme:widgets")), "package acme:widgets;\n");
}

#[test]
fn interface_functions_are_indented_and_terminated() {
    let mut package = WitPackage::new("acme:widgets");
    package.push_interface("renderer", vec![" draw: func() -> string ".to_string()]);
    let expected = "package acme:widgets;\n\ninterface renderer {\n  draw: func() -> string;\n}\n";
    assert_eq!(format_wit_package(&package), expected);
}
