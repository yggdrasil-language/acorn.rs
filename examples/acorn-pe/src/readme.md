# acorn-pe



`PE` / `COFF` / `ELF` 二进制格式 crate（Acorn 域）。由 `vcc-data::binary::{coff,elf,pe}` 逐步迁入，供 `nyar-emitter` 等消费者经 `acorn-pe` 编码，不再扩展 `vcc-data` 二进制面。



当前切片：`coff`、轻量 `elf` 读写、`pe` 头探测、`NativePeWriter` 与托管 `PeWriter`（含 `msil` 模型）。


