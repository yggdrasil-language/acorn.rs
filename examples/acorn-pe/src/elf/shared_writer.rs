use std::fmt;

use super::layout::{ELF_EHDR_SIZE, ELF_PHDR_SIZE};

/// 共享库 `ELF` 写出错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedElfWriteError {
    /// `.text` 节不能为空。
    EmptyText,
    /// 共享库至少需要一个导出符号。
    MissingExports,
    /// 宿主导入的 `rel32` 字段落在 `.text` 之外。
    ImportFixupOutOfRange {
        /// 声明的字段偏移。
        field: u32,
        /// `.text` 实际长度。
        text_len: u32,
    },
    /// 调用点引用了导入符号表之外的序号。
    ImportIndexOutOfRange {
        /// 调用点声明的导入序号。
        import: u32,
        /// 导入符号表长度。
        import_count: u32,
    },
    /// `.data` 自相对引用或重定位落在数据段之外。
    DataOutOfRange {
        /// 声明的数据段偏移。
        offset: u32,
        /// `.data` 实际长度。
        data_len: u32,
    },
    /// `rel32` 自相对位移超出可编码范围（镜像过大或布局非法）。
    Rel32Overflow {
        /// 发生溢出的字段偏移。
        field: u32,
    },
}

impl fmt::Display for SharedElfWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => write!(f, "`.text` 节不能为空"),
            Self::MissingExports => write!(f, "共享库至少需要一个导出符号"),
            Self::ImportFixupOutOfRange { field, text_len } => {
                write!(f, "宿主导入的 `rel32` 字段 {field} 超出 `.text` 长度 {text_len}")
            }
            Self::ImportIndexOutOfRange { import, import_count } => {
                write!(f, "调用点引用的导入序号 {import} 超出导入符号表长度 {import_count}")
            }
            Self::DataOutOfRange { offset, data_len } => {
                write!(f, "`.data` 偏移 {offset} 超出数据段长度 {data_len}")
            }
            Self::Rel32Overflow { field } => write!(f, "字段 {field} 的 `rel32` 自相对位移无法编码"),
        }
    }
}

impl std::error::Error for SharedElfWriteError {}

const ET_DYN: u16 = 3;
const EM_AARCH64: u16 = 183;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PF_R: u32 = 4;
const PF_W: u32 = 2;
const PF_X: u32 = 1;

const DT_NULL: u64 = 0;
const DT_HASH: u64 = 4;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const DT_STRSZ: u64 = 10;
const DT_SYMENT: u64 = 11;

/// `R_X86_64_GLOB_DAT`：把解析后的地址写入 GOT 槽（非惰性绑定）。
const R_X86_64_GLOB_DAT: u32 = 6;
/// `R_X86_64_RELATIVE`：把「装载基址 + 加数」写入目标字段（数据段自相对指针）。
const R_X86_64_RELATIVE: u32 = 8;

const STT_FUNC: u8 = 2;
const STB_GLOBAL: u8 = 1;
const STV_DEFAULT: u8 = 0;
const SHN_UNDEF: u16 = 0;
/// 定义符号所在的唯一节（本写入器不输出节头，宿主按动态表访问）。
const SHN_TEXT: u16 = 1;

/// 动态导出符号（`.text` 内偏移 + 名称）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedObjectExport {
    /// 导出符号名（如 `asgard_invoke_export`）。
    pub name: String,
    /// 函数在 `.text` 内的字节偏移。
    pub text_offset: u32,
}

/// 宿主导入调用点：`call [rip+disp32]` 的 `disp32` 字段位置 + 导入符号序号。
///
/// 导入符号表（`&[String]`）与调用点列表分离：同一个导入可以被多个调用点引用，
/// GOT 槽按**导入序号**唯一生成，调用点只携带序号，不重复声明符号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedElfImportCall {
    /// `call [rip+disp32]` 的 `disp32` 字段在 `.text` 内的偏移。
    pub rel32_field: u32,
    /// 导入符号表内的序号。
    pub import: u32,
}

/// `.data` 段内需要写入的自相对指针（`R_X86_64_RELATIVE`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedElfDataRelocation {
    /// 8 字节指针字段在 `.data` 内的偏移。
    pub offset: u32,
    /// 相对镜像基址的加数；镜像按位置无关布局（`p_vaddr == 文件偏移`）生成，
    /// 因此指向本镜像的 `.text`/`.data` 时直接取目标虚拟地址。
    pub addend: i64,
}

/// `.text` 内指向 `.data` 的自相对地址引用（`lea reg, [rip+disp32]` 等）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedElfDataReference {
    /// `disp32` 字段在 `.text` 内的偏移。
    pub rel32_field: u32,
    /// 目标在 `.data` 内的偏移。
    pub data_offset: u32,
}

/// `ET_DYN` 共享库镜像。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedElfImage {
    /// `.text` 节（只读可执行）。
    pub text: Vec<u8>,
    /// `.rodata` 节（可选；随首个只读段映射）。
    pub rodata: Vec<u8>,
    /// `.data` 节（可读写、文件内初始化；`p_vaddr == 文件偏移`）。
    pub data: Vec<u8>,
    /// `.bss` 节（可读写、文件内零填充）。
    pub bss: Vec<u8>,
}

/// `ET_DYN` 共享库写入器。
pub struct SharedElfWriter;

impl SharedElfWriter {
    /// 写出最小可加载 AArch64 `ET_DYN` `.so`（含 `.dynsym` 导出表）。
    pub fn write_aarch64(image: &SharedElfImage, exports: &[SharedObjectExport]) -> Result<Vec<u8>, SharedElfWriteError> {
        Self::write(image, exports, &[], &[], &[], &[], EM_AARCH64)
    }

    /// 写出最小可加载 x86-64 `ET_DYN` `.so`（含 `.dynsym` 导出表）。
    ///
    /// 镜像按位置无关布局生成：`p_vaddr` 与文件偏移恒等、装载基址为 0，导出的
    /// `st_value` 即 `.text` 内偏移。调用方只提供位置无关机器码（RIP 相对取址与
    /// 同镜像 `rel32` 调用），不需要动态重定位；宿主 `dlopen` + `dlsym` 直接取址。
    pub fn write_x86_64(image: &SharedElfImage, exports: &[SharedObjectExport]) -> Result<Vec<u8>, SharedElfWriteError> {
        Self::write(image, exports, &[], &[], &[], &[], EM_X86_64)
    }

    /// 写出带宿主导入与数据重定位的 x86-64 `ET_DYN` `.so`。
    ///
    /// - `imports`：未定义动态符号表，按序号各生成一个可写 GOT 槽与
    ///   `R_X86_64_GLOB_DAT` 重定位；
    /// - `calls`：`call [rip+disp32]` 调用点，按 `import` 序号指向对应 GOT 槽
    ///   （同一导入可有任意多个调用点）；
    /// - `data_relocations`：`.data` 内 `R_X86_64_RELATIVE` 自相对指针；
    /// - `data_references`：`.text` 内指向 `.data` 的 `rel32` 引用。
    #[allow(clippy::too_many_arguments)]
    pub fn write_x86_64_with_imports(
        image: &SharedElfImage,
        exports: &[SharedObjectExport],
        imports: &[String],
        calls: &[SharedElfImportCall],
        data_relocations: &[SharedElfDataRelocation],
        data_references: &[SharedElfDataReference],
    ) -> Result<Vec<u8>, SharedElfWriteError> {
        Self::write(image, exports, imports, calls, data_relocations, data_references, EM_X86_64)
    }

    /// 按机器类型写出 `ET_DYN`；AArch64 与 x86-64 共享同一动态表布局。
    #[allow(clippy::too_many_arguments)]
    fn write(
        image: &SharedElfImage,
        exports: &[SharedObjectExport],
        imports: &[String],
        calls: &[SharedElfImportCall],
        data_relocations: &[SharedElfDataRelocation],
        data_references: &[SharedElfDataReference],
        machine: u16,
    ) -> Result<Vec<u8>, SharedElfWriteError> {
        if image.text.is_empty() {
            return Err(SharedElfWriteError::EmptyText);
        }
        if exports.is_empty() {
            return Err(SharedElfWriteError::MissingExports);
        }
        for call in calls {
            if call.import as usize >= imports.len() {
                return Err(SharedElfWriteError::ImportIndexOutOfRange { import: call.import, import_count: imports.len() as u32 });
            }
        }

        let text_len = align_up(image.text.len() as u32, 16);
        let rodata_len = align_up(image.rodata.len() as u32, 16);
        let data_len = align_up(image.data.len() as u32, 16);
        let bss_len = align_up(image.bss.len() as u32, 16);
        let import_count = imports.len() as u32;

        // 可写段：`.data` 与 `.bss` 连续放置，段起点按页对齐，避免与只读段共享页
        // 后被整体改成可写（会丢掉 `.text` 的执行权限）。
        let rw_len = data_len + bss_len;
        let relocation_count = import_count + data_relocations.len() as u32;

        // 程序头：PT_LOAD +（可选数据 PT_LOAD）+（可选 GOT PT_LOAD）+ PT_DYNAMIC + PT_GNU_STACK。
        let phnum: u16 = 3 + u16::from(rw_len > 0) + u16::from(import_count > 0);
        let headers = ELF_EHDR_SIZE + ELF_PHDR_SIZE * u32::from(phnum);
        let text_off = align_up(headers, 16);
        let rodata_off = if rodata_len == 0 { 0 } else { text_off + text_len };
        let hash_off = text_off + text_len + rodata_len;

        let sym_count = exports.len() + imports.len() + 1;
        // SysV 哈希表：`nbucket=1`，`nchain=sym_count`，链覆盖全部符号（含 0 号空符号）。
        let hash_len = (8 + 4 + sym_count * 4) as u32;
        let dynsym_off = align_up(hash_off + hash_len, 8);
        let dynsym_len = (sym_count * 24) as u32;

        let mut dynstr = vec![0u8];
        let mut export_name_offsets = Vec::with_capacity(exports.len());
        for export in exports {
            export_name_offsets.push(dynstr.len() as u32);
            dynstr.extend_from_slice(export.name.as_bytes());
            dynstr.push(0);
        }
        let mut import_name_offsets = Vec::with_capacity(imports.len());
        for import in imports {
            import_name_offsets.push(dynstr.len() as u32);
            dynstr.extend_from_slice(import.as_bytes());
            dynstr.push(0);
        }
        let dynstr_len = dynstr.len() as u32;
        let dynstr_off = dynsym_off + dynsym_len;

        let rela_len = relocation_count * 24;
        let rela_off = if relocation_count == 0 { 0 } else { align_up(dynstr_off + dynstr_len, 8) };
        let dynamic_count = if relocation_count == 0 { 6 } else { 9 };
        let dynamic_len = dynamic_count * 16;
        let dynamic_off = align_up(if rela_off > 0 { rela_off + rela_len } else { dynstr_off + dynstr_len }, 8);
        let load_filesz = dynamic_off + dynamic_len;

        let rw_off = if rw_len == 0 { 0 } else { align_up(load_filesz, 0x1000) };
        // GOT 必须是可写页，且与只读段、数据段分页隔离。
        let got_off = if import_count == 0 {
            0
        }
        else if rw_len > 0 {
            align_up(rw_off + rw_len, 0x1000)
        }
        else {
            align_up(load_filesz, 0x1000)
        };
        let got_len = import_count * 8;
        let file_size = if got_off > 0 {
            got_off + got_len
        }
        else if rw_len > 0 {
            rw_off + rw_len
        }
        else {
            load_filesz
        };

        let load_vaddr: u64 = 0;

        let mut out = Vec::with_capacity(file_size as usize);
        write_ehdr(&mut out, phnum, machine);
        // 首个 `PT_LOAD` 覆盖头部、`.text`、`.rodata` 与全部动态表：`.hash`/`.dynsym`/
        // `.dynstr`/`.rela.dyn`/`.dynamic` 都靠虚地址访问，落在段外会让宿主读到未映射页。
        write_phdr_load(&mut out, 0, u64::from(load_filesz), load_vaddr, PF_R | PF_X);
        if rw_len > 0 {
            write_phdr_load(&mut out, u64::from(rw_off), u64::from(rw_len), u64::from(rw_off), PF_R | PF_W);
        }
        if got_off > 0 {
            write_phdr_load(&mut out, u64::from(got_off), u64::from(got_len), u64::from(got_off), PF_R | PF_W);
        }
        write_phdr_dynamic(&mut out, u64::from(dynamic_off), u64::from(dynamic_len), load_vaddr + u64::from(dynamic_off));
        // 显式声明**不可执行栈**：缺 PT_GNU_STACK 时 glibc 认为对象要求可执行栈，
        // 在加固宿主（含 Rust 测试线程）上 `_dl_make_stack_executable` 会失败，
        // 导致 `dlopen` 报 "cannot enable executable stack"。
        write_phdr_gnu_stack(&mut out);

        // `.text`：把每个导入调用点回填到对应 GOT 槽，把数据引用回填到 `.data` 目标。
        let mut text = image.text.clone();
        let fixup = |field: u32, target: i64, text: &mut Vec<u8>| -> Result<(), SharedElfWriteError> {
            let field_index = field as usize;
            if field_index + 4 > text.len() {
                return Err(SharedElfWriteError::ImportFixupOutOfRange { field, text_len: text.len() as u32 });
            }
            let field_vaddr = i64::from(text_off) + field as i64 + 4;
            let displacement = i32::try_from(target - field_vaddr).map_err(|_| SharedElfWriteError::Rel32Overflow { field })?;
            text[field_index..field_index + 4].copy_from_slice(&displacement.to_le_bytes());
            Ok(())
        };
        for call in calls {
            let slot_vaddr = i64::from(got_off) + 8 * i64::from(call.import);
            fixup(call.rel32_field, slot_vaddr, &mut text)?;
        }
        for reference in data_references {
            if reference.data_offset as usize >= image.data.len() {
                return Err(SharedElfWriteError::DataOutOfRange {
                    offset: reference.data_offset,
                    data_len: image.data.len() as u32,
                });
            }
            let target = i64::from(rw_off) + i64::from(reference.data_offset);
            fixup(reference.rel32_field, target, &mut text)?;
        }
        for relocation in data_relocations {
            if relocation.offset as usize + 8 > image.data.len() {
                return Err(SharedElfWriteError::DataOutOfRange {
                    offset: relocation.offset,
                    data_len: image.data.len() as u32,
                });
            }
        }
        pad_to(&mut out, text_off as usize);
        out.extend_from_slice(&text);
        pad_to(&mut out, (text_off + text_len) as usize);

        if rodata_len > 0 {
            out.extend_from_slice(&image.rodata);
            pad_to(&mut out, (rodata_off + rodata_len) as usize);
        }

        write_hash(&mut out, sym_count);
        pad_to(&mut out, dynsym_off as usize);
        write_dynsym(
            &mut out,
            exports,
            &export_name_offsets,
            imports,
            &import_name_offsets,
            load_vaddr + u64::from(text_off),
        );
        out.extend_from_slice(&dynstr);

        if rela_off > 0 {
            pad_to(&mut out, rela_off as usize);
            for (index, _) in imports.iter().enumerate() {
                let symbol = (exports.len() + index + 1) as u64;
                write_rela(&mut out, u64::from(got_off) + 8 * index as u64, (symbol << 32) | u64::from(R_X86_64_GLOB_DAT), 0);
            }
            for relocation in data_relocations {
                write_rela(&mut out, u64::from(rw_off) + u64::from(relocation.offset), u64::from(R_X86_64_RELATIVE), relocation.addend);
            }
        }

        pad_to(&mut out, dynamic_off as usize);
        write_dynamic(
            &mut out,
            load_vaddr + u64::from(hash_off),
            load_vaddr + u64::from(dynsym_off),
            load_vaddr + u64::from(dynstr_off),
            u64::from(dynstr_len),
            if rela_off > 0 { Some((load_vaddr + u64::from(rela_off), u64::from(rela_len))) } else { None },
        );

        if rw_len > 0 {
            pad_to(&mut out, rw_off as usize);
            out.extend_from_slice(&image.data);
            pad_to(&mut out, (rw_off + data_len) as usize);
            out.extend_from_slice(&image.bss);
            pad_to(&mut out, (rw_off + rw_len) as usize);
        }

        if got_off > 0 {
            pad_to(&mut out, got_off as usize);
            out.resize(got_off as usize + got_len as usize, 0);
        }

        debug_assert_eq!(out.len(), file_size as usize);
        Ok(out)
    }
}

fn write_ehdr(out: &mut Vec<u8>, phnum: u16, machine: u16) {
    out.extend_from_slice(&[0x7F, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&ET_DYN.to_le_bytes());
    out.extend_from_slice(&machine.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&(ELF_EHDR_SIZE as u64).to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(ELF_EHDR_SIZE as u16).to_le_bytes());
    out.extend_from_slice(&(ELF_PHDR_SIZE as u16).to_le_bytes());
    out.extend_from_slice(&phnum.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
}

/// `Elf64_Phdr` 的 `p_offset`/`p_vaddr`/`p_paddr`/`p_filesz`/`p_memsz` 都是 8 字节；
/// 少写半宽会让后续每个程序头整体错位，宿主按 `e_phoff` 解出无效段。
fn write_phdr_load(out: &mut Vec<u8>, offset: u64, filesz: u64, vaddr: u64, flags: u32) {
    out.extend_from_slice(&PT_LOAD.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&vaddr.to_le_bytes());
    out.extend_from_slice(&vaddr.to_le_bytes());
    out.extend_from_slice(&filesz.to_le_bytes());
    out.extend_from_slice(&filesz.to_le_bytes());
    out.extend_from_slice(&0x1000u64.to_le_bytes());
}

/// `PT_GNU_STACK`：文件不占用、运行时按 `PF_R | PF_W`（不可执行）的栈。
fn write_phdr_gnu_stack(out: &mut Vec<u8>) {
    out.extend_from_slice(&PT_GNU_STACK.to_le_bytes());
    out.extend_from_slice(&(PF_R | PF_W).to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&16u64.to_le_bytes());
}

fn write_phdr_dynamic(out: &mut Vec<u8>, offset: u64, filesz: u64, vaddr: u64) {
    out.extend_from_slice(&PT_DYNAMIC.to_le_bytes());
    out.extend_from_slice(&(PF_R).to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&vaddr.to_le_bytes());
    out.extend_from_slice(&vaddr.to_le_bytes());
    out.extend_from_slice(&filesz.to_le_bytes());
    out.extend_from_slice(&filesz.to_le_bytes());
    out.extend_from_slice(&0x8u64.to_le_bytes());
}

/// SysV 哈希表：单桶线性链覆盖全部符号。
///
/// `nchain` 与 `.dynsym` 项数相同；`chain[0]` 是空符号、永不被遍历，`chain[i]`
/// 指向下一个同桶符号（最后一个为 0）。只写单桶是对的：本写入器不生成哈希值，
/// 所有名字都落在 0 号桶，宿主查找退化为符号表线性扫描。
fn write_hash(out: &mut Vec<u8>, sym_count: usize) {
    let count = sym_count as u32;
    out.extend_from_slice(&1u32.to_le_bytes()); // nbucket
    out.extend_from_slice(&count.to_le_bytes()); // nchain
    // 0 号桶指向第一个导出符号；只有空符号时为空。
    out.extend_from_slice(&u32::from(count > 1).to_le_bytes());
    for index in 0..count {
        let next = if index > 0 && index + 1 < count { index + 1 } else { 0 };
        out.extend_from_slice(&next.to_le_bytes());
    }
}

fn write_dynsym(
    out: &mut Vec<u8>,
    exports: &[SharedObjectExport],
    export_name_offsets: &[u32],
    imports: &[String],
    import_name_offsets: &[u32],
    text_vaddr: u64,
) {
    write_sym(out, 0, 0, 0, SHN_UNDEF, 0);
    for (export, name_off) in exports.iter().zip(export_name_offsets) {
        let value = text_vaddr + u64::from(export.text_offset);
        write_sym(out, *name_off, (STB_GLOBAL << 4) | STT_FUNC, STV_DEFAULT, SHN_TEXT, value);
    }
    for name_off in import_name_offsets.iter().take(imports.len()) {
        write_sym(out, *name_off, (STB_GLOBAL << 4) | STT_FUNC, STV_DEFAULT, SHN_UNDEF, 0);
    }
}

fn write_sym(out: &mut Vec<u8>, name: u32, info: u8, other: u8, shndx: u16, value: u64) {
    out.extend_from_slice(&name.to_le_bytes());
    out.push(info);
    out.push(other);
    out.extend_from_slice(&shndx.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
}

/// `Elf64_Rela`：`r_offset`、`r_info = (sym << 32) | type`、`r_addend`。
fn write_rela(out: &mut Vec<u8>, offset: u64, info: u64, addend: i64) {
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&info.to_le_bytes());
    out.extend_from_slice(&addend.to_le_bytes());
}

fn write_dynamic(
    out: &mut Vec<u8>,
    hash_vaddr: u64,
    symtab_vaddr: u64,
    strtab_vaddr: u64,
    strsz: u64,
    rela: Option<(u64, u64)>,
) {
    write_dyn(out, DT_HASH, hash_vaddr);
    write_dyn(out, DT_SYMTAB, symtab_vaddr);
    write_dyn(out, DT_STRTAB, strtab_vaddr);
    write_dyn(out, DT_STRSZ, strsz);
    write_dyn(out, DT_SYMENT, 24);
    if let Some((rela_vaddr, rela_size)) = rela {
        write_dyn(out, DT_RELA, rela_vaddr);
        write_dyn(out, DT_RELASZ, rela_size);
        write_dyn(out, DT_RELAENT, 24);
    }
    write_dyn(out, DT_NULL, 0);
}

fn write_dyn(out: &mut Vec<u8>, tag: u64, value: u64) {
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

fn align_up(value: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return value;
    }
    let rem = value % alignment;
    if rem == 0 { value } else { value + (alignment - rem) }
}

fn pad_to(out: &mut Vec<u8>, offset: usize) {
    while out.len() < offset {
        out.push(0);
    }
}
