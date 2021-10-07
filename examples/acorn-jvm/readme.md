# acorn-jvm

JVM `class` / `JAR` 二进制格式 crate（Acorn 域）。

## 🎯 职责

- **`class`**：`JVM class` 文件模型与编解码。以 `0xCAFEBABE` 魔数开始，覆盖常量池、类型描述符 `JvmTypeDescriptor`、成员与访问标志（`ACC_PUBLIC` / `ACC_STATIC` / `ACC_SUPER` / `ACC_NATIVE` / `ACC_ABSTRACT`）等模型，详见 [`class`](crate::class)。
- **`jar`**：`JAR` 归档打包模型与产物边界。容器读写走 `acorn-zip`，写出为 `ZIP32` stored 成员，详见 [`jar`](crate::jar)。

## 🔗 依赖

- 容器层复用 `acorn-zip`，不在本 crate 内另写 ZIP 结构。
- 顶层不启用压缩，`class` / `jar` 只处理各自的字节结构与打包布局。

## 🚧 边界

- 只负责 `class` / `JAR` 的字节结构与编码，不承载 JVM 校验、链接或执行语义。
- `DEX` 等相邻格式归 `acorn-dex`，本 crate 不扩展 Java 之外的平台格式。

## ⚠️ 错误

`JvmClassError` 区分 `UnexpectedEof`、`InvalidMagic`（魔数不符）、`UnsupportedConstantTag`（常量池标签不支持）、`InvalidConstantReference`（常量池引用越界或类型不匹配）与 `InvalidFormat`。

## 🤝 贡献

欢迎提交改进！新增常量池标签或成员支持时，先补错误类别与结构测试，不要把未支持的标签静默跳过。
