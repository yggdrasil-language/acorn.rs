# acorn-dex

Android `DEX`（`dex/035`）二进制格式 crate（Acorn 域）。

## 职责

- `dex::writer`：把 JVM `.class` 字节编入可解析的 `classes.dex`，支持附加尾段。
- `dex::reader`：`string_ids` / `type_ids` / `class_defs` 最小读取与字符串扫描。
- `dex::merge`：多组 class 集合与多镜像合并。

## 边界

- 只负责 `DEX` 字节结构，不承载语言语义。
- JVM `.class` 编解码由 `acorn-jvm` 拥有。
- 尾段（`ASGARDNT` / `ASGARDUI`）在本 crate 内只是不透明字节，解析归装配层。
