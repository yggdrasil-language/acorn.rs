# acorn-mach-o

Apple `Mach-O` 二进制格式 crate（Acorn 域）。

## 职责

- `mach_o`：最小 64 位 `Mach-O`（`aarch64` `MH_EXECUTE`）镜像写出，含 `LC_SEGMENT_64 __TEXT` 与 `LC_MAIN`。

## 边界

- 只负责 `Mach-O` 字节结构，不承载语言语义；`__TEXT` 代码段由装配层提供字节并在本 crate 内替换。
