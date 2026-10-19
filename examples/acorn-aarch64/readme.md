# acorn-aarch64

AArch64 指令编码与 Android JNI 胶水 crate（Acorn 域）。

## 职责

- `aarch64`：`A64Instruction` 常量、两遍编码器与 fixup、`RegX`、AAPCS64 函数构建器。
- `aarch64::jni_glue`：`JNI_OnLoad` / `asgard_invoke_export` 胶水机器码，以及把胶水与逻辑段合并为 `ET_DYN` 共享库镜像。

## 边界

- 只负责机器码字节与导出符号偏移，不承载语言语义。
- `ELF64` `ET_DYN` 容器写出由 `acorn-pe` 拥有；本 crate 只构造其镜像与导出表。
