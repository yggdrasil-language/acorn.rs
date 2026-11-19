//! MSIL 文本写出用例：独立期望文本，不复用实现内部结构。

use acorn_pe::msil::{
    MsilAssembly, MsilInstruction, MsilInstructionOperand, MsilMethodBody, MsilMethodRef, MsilMethodSignature, MsilModule,
    MsilOpcode, MsilTextWriter, MsilType, MsilTypeDef,
};

const INT32: MsilType = MsilType::Int32 { signed: true };

fn instruction(opcode: MsilOpcode) -> MsilInstruction {
    MsilInstruction { label: None, opcode, operand: None }
}

fn add_method() -> MsilMethodBody {
    MsilMethodBody {
        method: MsilMethodRef {
            owner: None,
            name: "add".to_string(),
            signature: MsilMethodSignature::new(INT32, vec![INT32, INT32]),
        },
        locals: vec![INT32],
        instructions: vec![instruction(MsilOpcode::LdcI4_1), instruction(MsilOpcode::LdcI4_2), instruction(MsilOpcode::Ret)],
        max_stack: 2,
        is_entry_point: true,
        is_async: false,
    }
}

fn demo_module() -> MsilModule {
    MsilModule {
        assembly: MsilAssembly { name: "Demo".to_string(), externs: vec!["mscorlib".to_string()] },
        types: vec![MsilTypeDef {
            full_name: "core.text.TextSpan".to_string(),
            namespace: "core.text".to_string(),
            fields: Vec::new(),
            methods: vec![add_method()],
            is_value_type: true,
        }],
        global_methods: Vec::new(),
    }
}

#[test]
fn writes_assembly_type_and_method_body_in_order() {
    let text = MsilTextWriter::new().write_module(&demo_module());
    let expected = concat!(
        ".assembly extern mscorlib {}\n",
        "\n",
        ".assembly Demo {}\n",
        "\n",
        ".class public auto ansi beforefieldinit core.text.TextSpan\n",
        "{\n",
        "    .method public hidebysig static int32 add(int32, int32) cil managed\n",
        "    {\n",
        "        .entrypoint\n",
        "        .locals init (int32)\n",
        "        .maxstack 2\n",
        "        ldc.i4.1\n",
        "        ldc.i4.2\n",
        "        ret\n",
        "    }\n",
        "}\n",
        "\n",
    );
    assert_eq!(text, expected);
}

#[test]
fn custom_indent_text_replaces_each_level() {
    let text = MsilTextWriter::new().with_indent_text("\t").write_module(&demo_module());
    assert!(text.contains("\n\t.method public hidebysig"), "{text}");
    assert!(text.contains("\n\t\t.entrypoint"), "{text}");
    assert!(!text.contains("    "), "default indent must not leak: {text}");
}

#[test]
fn module_file_name_is_written_before_types() {
    let text = MsilTextWriter::new().write_module_with_file_name(&demo_module(), "Demo.dll");
    assert!(text.starts_with(".assembly extern mscorlib {}"), "{text}");
    assert!(text.contains(".module Demo.dll\n"), "{text}");
    assert!(text.find(".module Demo.dll").unwrap() < text.find(".class public").unwrap(), "{text}");
}

#[test]
fn operands_are_rendered_from_their_variant() {
    let mut method = add_method();
    method.instructions = vec![
        MsilInstruction {
            label: Some("L0".to_string()),
            opcode: MsilOpcode::LdcI4S,
            operand: Some(MsilInstructionOperand::Integer(7)),
        },
        MsilInstruction {
            label: None,
            opcode: MsilOpcode::Ldstr,
            operand: Some(MsilInstructionOperand::StringLiteral("a\"b".to_string())),
        },
    ];
    let module = MsilModule {
        assembly: MsilAssembly { name: "Demo".to_string(), externs: Vec::new() },
        types: Vec::new(),
        global_methods: vec![method],
    };
    let text = MsilTextWriter::new().write_module(&module);
    assert!(text.contains("    L0:\n"), "{text}");
    assert!(text.contains("    ldc.i4.s 7\n"), "{text}");
    assert!(text.contains("    ldstr \"a\\\"b\"\n"), "{text}");
}
