mod layout;

use std::fmt;

pub use self::layout::NativeSectionLayout;
use self::layout::SectionPlacement;
pub use super::writer::IMAGE_BASE_X64;
pub(crate) use super::writer::{FILE_ALIGNMENT, PeBinaryWriter, SECTION_ALIGNMENT};

/// 原生 `PE` 写出错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePeWriteError {
    /// `.text` 节不能为空。
    EmptyText,
    /// 节或导入目录过大。
    SizeOverflow(&'static str),
}

impl fmt::Display for NativePeWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => write!(f, "`.text` 节不能为空"),
            Self::SizeOverflow(what) => write!(f, "{what} 过大"),
        }
    }
}

impl std::error::Error for NativePeWriteError {}

/// 单个 `DLL` 导入描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDllImport {
    /// `DLL` 名称。
    pub dll: String,
    /// 导入函数名。
    pub functions: Vec<String>,
}

/// 单个导出符号描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExport {
    /// 导出名。
    pub name: String,
    /// 符号在 `.text` 内的偏移。
    pub offset: u32,
}

/// 待写入的原生 `PE` 镜像。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePeImage {
    /// `.text` 节。
    pub text: Vec<u8>,
    /// `.rdata` 节。
    pub rdata: Vec<u8>,
    /// `.idata` 节（段内偏移尚未加 `idata_rva`）。
    pub idata: Vec<u8>,
    /// 导入目录描述。
    pub imports: Vec<NativeDllImport>,
    /// 导出符号（写入 `.edata`；非空时镜像必须按 DLL 写出）。
    pub exports: Vec<NativeExport>,
    /// 是否按 `DLL` 写出（`DLL` 标志、无入口点）。
    pub is_dll: bool,
    /// 入口点在 `.text` 内的偏移。
    pub entry_point: u32,
}

/// 原生 `PE` 可执行文件写入器。
pub struct NativePeWriter;

impl NativePeWriter {
    /// 将原生镜像写入 `PE32+` 控制台可执行文件。
    pub fn write_executable(image: &NativePeImage) -> Result<Vec<u8>, NativePeWriteError> {
        if image.text.is_empty() {
            return Err(NativePeWriteError::EmptyText);
        }

        let text_len = u32::try_from(image.text.len()).map_err(|_| NativePeWriteError::SizeOverflow("`.text`"))?;
        let rdata_len = u32::try_from(image.rdata.len()).map_err(|_| NativePeWriteError::SizeOverflow("`.rdata`"))?;
        let idata_len = u32::try_from(image.idata.len()).map_err(|_| NativePeWriteError::SizeOverflow("`.idata`"))?;
        let mut edata = build_export_directory(&image.exports);
        let edata_len = u32::try_from(edata.len()).map_err(|_| NativePeWriteError::SizeOverflow("`.edata`"))?;
        let layout = NativeSectionLayout::compute_with_exports(text_len, rdata_len, idata_len, edata_len);

        let mut idata = image.idata.clone();
        let import_directory_size = if image.imports.is_empty() {
            0
        }
        else {
            u32::try_from((image.imports.len() + 1) * 20).map_err(|_| NativePeWriteError::SizeOverflow("导入目录"))?
        };
        let import_directory_rva = layout.idata.map(|section| section.rva).unwrap_or(0);
        if let Some(idata_section) = layout.idata {
            patch_idata_rvas(&mut idata, idata_section.rva, image.imports.len());
        }
        let export_directory_size = if image.exports.is_empty() { 0 } else { 40 };
        if let (Some(edata_section), false) = (layout.edata, image.exports.is_empty()) {
            patch_edata_rvas(&mut edata, edata_section.rva, layout.text.rva, &image.exports);
        }
        let export_directory_rva = layout.edata.map(|section| section.rva).unwrap_or(0);

        let entry_rva = if image.is_dll { 0 } else { layout.text.rva + image.entry_point };
        let mut writer = PeBinaryWriter::new();
        write_dos_header(&mut writer);
        writer.write_u32(0x0000_4550);
        write_coff_header(&mut writer, layout.section_count, image.is_dll);
        write_optional_header(
            &mut writer,
            layout.text.rva,
            layout.text.raw_size,
            layout.size_of_initialized_data,
            layout.size_of_headers,
            layout.size_of_image,
            entry_rva,
            export_directory_rva,
            export_directory_size,
            import_directory_rva,
            import_directory_size,
        );
        write_section(&mut writer, b".text\0\0\0", &layout.text, 0x6000_0020);
        if let Some(rdata) = layout.rdata {
            write_section(&mut writer, b".rdata\0\0", &rdata, 0x4000_0040);
        }
        if let Some(idata_section) = layout.idata {
            write_section(&mut writer, b".idata\0\0", &idata_section, 0xC000_0040);
        }
        if let Some(edata_section) = layout.edata {
            write_section(&mut writer, b".edata\0\0", &edata_section, 0x4000_0040);
        }

        pad_to(&mut writer, layout.size_of_headers as usize);
        writer.write_bytes(&image.text);
        pad_to(&mut writer, (layout.text.raw_ptr + layout.text.raw_size) as usize);
        if let Some(rdata) = layout.rdata {
            writer.write_bytes(&image.rdata);
            pad_to(&mut writer, (rdata.raw_ptr + rdata.raw_size) as usize);
        }
        if let Some(idata_section) = layout.idata {
            writer.write_bytes(&idata);
            pad_to(&mut writer, (idata_section.raw_ptr + idata_section.raw_size) as usize);
        }
        if let Some(edata_section) = layout.edata {
            writer.write_bytes(&edata);
            pad_to(&mut writer, (edata_section.raw_ptr + edata_section.raw_size) as usize);
        }

        Ok(writer.into_bytes())
    }
}

/// 构造 `.edata` 导出目录（段内 RVA 仍是相对偏移，由写入器加 `edata_rva`）。
///
/// 布局：40 字节目录头 → 函数 RVA 表 → 名称 RVA 表 → 序号表 → 名称字符串。
/// 名称按 ASCII 升序排列，满足加载器二分查找要求。
fn build_export_directory(exports: &[NativeExport]) -> Vec<u8> {
    if exports.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<&NativeExport> = exports.iter().collect();
    sorted.sort_by(|left, right| left.name.cmp(&right.name));

    let count = sorted.len();
    let functions_offset = 40usize;
    let names_offset = functions_offset + count * 4;
    let ordinals_offset = names_offset + count * 4;
    let strings_offset = ordinals_offset + count * 2;

    let mut strings = Vec::new();
    strings.extend_from_slice(b"nyar_residual\0");
    let mut name_offsets = Vec::with_capacity(count);
    for export in &sorted {
        name_offsets.push(strings.len());
        strings.extend_from_slice(export.name.as_bytes());
        strings.push(0);
    }

    let mut data = vec![0u8; strings_offset + strings.len()];
    // 模块名 RVA（相对 `.edata` 起点）指向字符串区首字节。
    put_u32(&mut data, 12, strings_offset as u32);
    put_u32(&mut data, 16, 1);
    put_u32(&mut data, 20, count as u32);
    put_u32(&mut data, 24, count as u32);
    put_u32(&mut data, 28, functions_offset as u32);
    put_u32(&mut data, 32, names_offset as u32);
    put_u32(&mut data, 36, ordinals_offset as u32);
    for (index, export) in sorted.iter().enumerate() {
        put_u32(&mut data, functions_offset + index * 4, export.offset);
        put_u32(&mut data, names_offset + index * 4, (strings_offset + name_offsets[index]) as u32);
        put_u16(&mut data, ordinals_offset + index * 2, index as u16);
    }
    data[strings_offset..].copy_from_slice(&strings);
    data
}

/// 把 `.edata` 内的相对偏移加上对应节 RVA。
///
/// 目录内 RVA（模块名、三张表）相对 `.edata`；函数 RVA 必须指向 `.text`。
fn patch_edata_rvas(edata: &mut [u8], edata_rva: u32, text_rva: u32, exports: &[NativeExport]) {
    let count = exports.len();
    let add_rva = |edata: &mut [u8], offset: usize, base: u32| {
        let value = u32::from_le_bytes(edata[offset..offset + 4].try_into().unwrap());
        edata[offset..offset + 4].copy_from_slice(&(base + value).to_le_bytes());
    };
    // 模块名 RVA、函数表/名称表/序号表 RVA。
    for offset in [12usize, 28, 32, 36] {
        add_rva(edata, offset, edata_rva);
    }
    for index in 0..count {
        add_rva(edata, 40 + index * 4, text_rva);
        add_rva(edata, 40 + count * 4 + index * 4, edata_rva);
    }
}

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn patch_idata_rvas(idata: &mut [u8], idata_rva: u32, import_count: usize) {
    let mut thunk_starts = Vec::with_capacity(import_count * 2);
    for index in 0..import_count {
        let offset = index * 20;
        let ilt = u32::from_le_bytes(idata[offset..offset + 4].try_into().unwrap());
        let name = u32::from_le_bytes(idata[offset + 12..offset + 16].try_into().unwrap());
        let iat = u32::from_le_bytes(idata[offset + 16..offset + 20].try_into().unwrap());
        thunk_starts.push(ilt);
        thunk_starts.push(iat);
        idata[offset..offset + 4].copy_from_slice(&(idata_rva + ilt).to_le_bytes());
        idata[offset + 12..offset + 16].copy_from_slice(&(idata_rva + name).to_le_bytes());
        idata[offset + 16..offset + 20].copy_from_slice(&(idata_rva + iat).to_le_bytes());
    }

    for start in thunk_starts {
        let mut cursor = start as usize;
        while cursor + 8 <= idata.len() {
            let value = u64::from_le_bytes(idata[cursor..cursor + 8].try_into().unwrap());
            if value == 0 {
                break;
            }
            idata[cursor..cursor + 8].copy_from_slice(&(idata_rva as u64 + value).to_le_bytes());
            cursor += 8;
        }
    }
}

fn write_dos_header(writer: &mut PeBinaryWriter) {
    writer.write_u16(0x5A4D);
    for _ in 0..29 {
        writer.write_u16(0);
    }
    writer.write_u32(0x40);
}

fn write_coff_header(writer: &mut PeBinaryWriter, section_count: u16, is_dll: bool) {
    writer.write_u16(0x8664);
    writer.write_u16(section_count);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u16(240);
    // EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE | RELOCS_STRIPPED（DLL 另加 DLL 标志）
    writer.write_u16(if is_dll { 0x2022 } else { 0x0023 });
}

#[allow(clippy::too_many_arguments)]
fn write_optional_header(
    writer: &mut PeBinaryWriter,
    text_rva: u32,
    text_raw_size: u32,
    size_of_initialized_data: u32,
    size_of_headers: u32,
    size_of_image: u32,
    entry_rva: u32,
    export_rva: u32,
    export_size: u32,
    import_rva: u32,
    import_size: u32,
) {
    writer.write_u16(0x020B);
    writer.write_u8(14);
    writer.write_u8(0);
    writer.write_u32(text_raw_size);
    writer.write_u32(size_of_initialized_data);
    writer.write_u32(0);
    writer.write_u32(entry_rva);
    writer.write_u32(text_rva);
    writer.write_u64(IMAGE_BASE_X64);
    writer.write_u32(SECTION_ALIGNMENT);
    writer.write_u32(FILE_ALIGNMENT);
    writer.write_u16(6);
    writer.write_u16(0);
    writer.write_u16(0);
    writer.write_u16(0);
    writer.write_u16(6);
    writer.write_u16(0);
    writer.write_u32(0);
    writer.write_u32(size_of_image);
    writer.write_u32(size_of_headers);
    writer.write_u32(0);
    writer.write_u16(3);
    // NX_COMPAT only (no DYNAMIC_BASE — 无 .reloc)
    writer.write_u16(0x0100);
    writer.write_u64(0x100000);
    writer.write_u64(0x1000);
    writer.write_u64(0x100000);
    writer.write_u64(0x1000);
    writer.write_u32(0);
    writer.write_u32(16);
    // Export
    writer.write_u32(export_rva);
    writer.write_u32(export_size);
    // Import
    writer.write_u32(import_rva);
    writer.write_u32(import_size);
    for _ in 0..14 {
        writer.write_u32(0);
        writer.write_u32(0);
    }
}

fn write_section(writer: &mut PeBinaryWriter, name: &[u8; 8], section: &SectionPlacement, characteristics: u32) {
    writer.write_bytes(name);
    writer.write_u32(section.virtual_size);
    writer.write_u32(section.rva);
    writer.write_u32(section.raw_size);
    writer.write_u32(section.raw_ptr);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u16(0);
    writer.write_u16(0);
    writer.write_u32(characteristics);
}

fn pad_to(writer: &mut PeBinaryWriter, size: usize) {
    while writer.len() < size {
        writer.write_u8(0);
    }
}
