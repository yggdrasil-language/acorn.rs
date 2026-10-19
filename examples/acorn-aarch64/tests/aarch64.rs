//! `aarch64` 最小指令编码。

use acorn_aarch64::aarch64::{A64Encoder, A64Instruction, RegX, encode_instructions, ret_bytes};

#[test]
fn ret_opcode_is_four_bytes() {
    assert_eq!(ret_bytes(), [0xC0, 0x03, 0x5F, 0xD6]);
}

#[test]
fn encode_instructions_is_little_endian() {
    let bytes = encode_instructions(&[0xD65F_03C0, 0xD503_201F]);
    assert_eq!(bytes, vec![0xC0, 0x03, 0x5F, 0xD6, 0x1F, 0x20, 0x03, 0xD5]);
}

#[test]
fn encodes_register_moves() {
    let mut encoder = A64Encoder::new();
    encoder.push(A64Instruction::MovRegReg { dst: RegX::X0, src: RegX::X1 });
    let module = encoder.finish();
    assert_eq!(module.text.len(), 4);
}
