# acorn-host

各平台最小宿主可执行封装 crate（Acorn 域）。

## 🎯 职责

- `host_exe::build_host_executable(platform, native_logic, rodata_tail)`：按平台分派到对应容器编码器，产出可承载宿主原生逻辑与只读尾段（`rodata_tail`，如 Asgard UI wire 段）的最小可执行字节。
- 平台分派：
  - `"windows"` → `PE32+`（经 `acorn-x64::native_pe::NativeImageBuilder`，把原生逻辑与尾段放入 `.rdata`，补一个 `_start` 入口）。
  - `"linux"` → `ELF64`（经 `acorn-x64::native_elf::NativeElfImageBuilder`）。
  - 其他平台 → `Mach-O`（经 `acorn-mach-o::mach_o::MachOImageBuilder`）。
- `native_logic` 为 `None` 时只写出尾段占位入口；`rodata_tail` 为空时不追加只读段。

## 🏗️ 依赖方向

```text
acorn-host → acorn-x64（native_pe / native_elf / x86_64） + acorn-mach-o
```

## 🚧 边界

- 只做「平台 → 容器编码器」的分发与最小入口编排，不承载语言语义，也不做链接、加载或执行。
- 各容器的字节编码由 `acorn-x64`（`PE32+` / `ELF64`）与 `acorn-mach-o` 拥有；本 crate 只在其公开镜像构建器上做编排。
- 尾段的语义（UI wire、资源材料化）由上层装配方拥有，本 crate 只把它当作不透明字节写入只读段。

## 🤝 贡献

欢迎提交改进！新增平台时必须复用既有容器编码器，不要在本 crate 内重写容器布局。
