use acorn_wasm::opcode::WasmOpcode;

#[test]
fn i64_mvp_opcodes_roundtrip() {
    assert_eq!(WasmOpcode::I64ShrU.as_u8(), 0x88);
    assert_eq!(WasmOpcode::I64ExtendI32S.as_u8(), 0xAC);
    assert_eq!(WasmOpcode::from_u8(0x88), Some(WasmOpcode::I64ShrU));
    assert_eq!(WasmOpcode::from_u8(0xAC), Some(WasmOpcode::I64ExtendI32S));
    assert_eq!(WasmOpcode::I64ShrU.mnemonic(), "i64.shr_u");
    assert_eq!(WasmOpcode::I64ExtendI32S.mnemonic(), "i64.extend_i32_s");
}
