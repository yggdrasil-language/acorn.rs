# acorn-host

各平台最小宿主可执行封装 crate（Acorn 域）。

## 职责

- `host_exe::build_host_executable`：按 `windows` / `linux` / 其他平台分别选 `PE32+`、`ELF64`、`Mach-O` 容器，产出可承载宿主原生逻辑与只读尾段的最小可执行字节。

## 边界

- 只做平台到容器编码器的分发，不承载语言语义。
- 各容器的编码由 `acorn-x64`（`PE32+` / `ELF64`）与 `acorn-mach-o` 拥有。
