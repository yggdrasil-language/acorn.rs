# acorn-x64

x86-64 指令编码与原生宿主镜像 crate（Acorn 域）。

## 职责

- `x86_64`：`X64Instruction` IR、两遍编码器与 fixup 回填、`Reg64` / `ConditionCode`、MSVC / SysV 调用约定函数构建器、`EncodedModule` → `COFF` 对象布局。
- `native_pe`：把编码后的 `.text`、`.rdata`、导入表编排为 `PE32+` 可执行镜像并写出。
- `native_elf`：把编码后的 `.text`、`.rodata` 编排为 `ELF64` `ET_EXEC` 或 AArch64 `ET_DYN` 共享库并写出。

## 边界

- 只负责机器码字节与容器布局，不承载语言语义。
- `PE` / `COFF` / `ELF` 的容器写出由 `acorn-pe` 拥有；本 crate 只在其公开镜像类型与布局表上做编排。
