//! `x86_64` 编码器：最小 stub 字节与标签偏移。

use acorn_x64::x86_64::{Reg64, X64Encoder, X64Instruction};

#[test]
fn encodes_return_zero_stub() {
    let mut encoder = X64Encoder::new();
    encoder.push(X64Instruction::XorReg { dst: Reg64::Rax, src: Reg64::Rax });
    encoder.push(X64Instruction::Ret);
    let module = encoder.finish();
    assert_eq!(module.text, vec![0x31, 0xC0, 0xC3]);
}

#[test]
fn records_label_offsets() {
    let mut encoder = X64Encoder::new();
    encoder.push(X64Instruction::Label("entry".to_string()));
    encoder.push(X64Instruction::Ret);
    let module = encoder.finish();
    assert_eq!(module.labels.get("entry"), Some(&0));
}
