# acorn-wasm

`WebAssembly` 二进制模块模型（Acorn 域）。

## 🎯 职责

- **模块级**：`WasmBinaryModule` 持有魔数校验（`\0asm`）与版本（当前只支持 `1`）、段列表 `WasmSection` 与自定义段视图 `WasmCustomSection`，提供 `from_bytes` / `to_bytes` 双向转换。
- **段级**：`parse_type_section` / `parse_import_section` / `parse_export_section` / `parse_code_section` 解析各段模型（`WasmTypeEntry`、`WasmImport`、`WasmExport`、`WasmFunctionEntry`），`read_value_type` / `read_heap_type` 解析值类型与堆类型，`section_name` 给出段 `id` 的可读名，`compute_section_offset` 计算段 `payload` 在文件中的绝对偏移。
- **指令级**：`WasmOpcode` / `WasmGcOpcode` / `WasmMiscOpcode` 操作码表与助记符查表，`DecodedInstruction` / `DecodedOperand` / `WasmPrefix` 解码模型，`decode_instruction` / `decode_gc_instruction` / `decode_code_body` 指令序列解码。
- **编码辅助**：`encode_*` 一族（调用、分支、局部变量、内存、`GC` 结构/数组、`ref` 系列等）与 `encode_functype` / `encode_structtype` / `encode_arraytype` 类型段编码，供生产者复用。

## 🔗 依赖

- `acorn-codec::integer::Leb128` 提供 `ULEB128` / `SLEB128` 编解码，不重复实现变长整数。
- 只依赖 `acorn-codec` 的整数变换，不启用 `compression`，不读取压缩段数据。

## 🚧 边界

- 只负责 `WASM` 二进制结构与指令字节，不持有验证、实例化或执行语义。
- 只接受版本 `1`，遇到其它版本返回 `WasmBinaryError::UnsupportedVersion`。
- 自定义段内容按不透明字节保留，段语义由上层消费者解释。

## ⚠️ 错误

`WasmBinaryError` 区分 `UnexpectedEof`（中途结束）、`InvalidMagic`（魔数不符）、`UnsupportedVersion`、`InvalidLeb128`（变长整数不合法）与 `InvalidUtf8Name`（自定义段名不是有效 UTF-8），并实现 `Display` 与 `std::error::Error`。

## 🤝 贡献

欢迎提交改进！新增指令或段支持时，必须复用 `acorn-codec` 的变长整数入口，不要在 crate 内另写 `LEB128` 解析。
