#![doc = include_str!("readme.md")]

use std::{
    fmt::{Display, Formatter},
    io,
};

use acorn_core::Budget;
use acorn_source::{ByteSource, MemorySource, ReadOutcome};
use acorn_zip::{ZipIndexError, ZipViewError, decode_member, index_zip_bytes};
use serde::{Deserialize, Serialize};

use crate::class::{JvmClassError, JvmClassFile};

const MANIFEST_PATH: &str = "META-INF/MANIFEST.MF";
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const EOCD_SIGNATURE: u32 = 0x0605_4b50;

/// `JAR` 读写错误。
#[derive(Debug)]
pub enum JvmJarError {
    /// 归档不是可索引的 ZIP。
    Index(ZipIndexError),
    /// 成员解码失败。
    Decode(ZipViewError),
    /// 底层 `IO` 失败。
    Io(io::Error),
    /// `ClassFile` 编解码失败。
    Class(JvmClassError),
    /// 名称或长度超出 ZIP32 stored 成员范围。
    Invalid(String),
}

impl Display for JvmJarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Index(error) => write!(f, "JAR ZIP 索引失败：{error}"),
            Self::Decode(error) => write!(f, "JAR 成员解码失败：{error}"),
            Self::Io(error) => write!(f, "JAR IO 失败：{error}"),
            Self::Class(error) => write!(f, "JAR 中的 class 文件无效：{error}"),
            Self::Invalid(message) => write!(f, "JAR 条目无效：{message}"),
        }
    }
}

impl std::error::Error for JvmJarError {}

impl From<ZipIndexError> for JvmJarError {
    fn from(value: ZipIndexError) -> Self {
        Self::Index(value)
    }
}

impl From<ZipViewError> for JvmJarError {
    fn from(value: ZipViewError) -> Self {
        Self::Decode(value)
    }
}

impl From<io::Error> for JvmJarError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<JvmClassError> for JvmJarError {
    fn from(value: JvmClassError) -> Self {
        Self::Class(value)
    }
}

/// `JAR` 归档入口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct JvmJarEntry {
    /// 归档内路径。
    pub path: String,
    /// 入口内容。
    pub bytes: Vec<u8>,
}

/// `JAR` 产物模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct JvmJarPackage {
    /// 归档文件名。
    pub file_name: String,
    /// `MANIFEST.MF` 主类。
    pub main_class: Option<String>,
    /// 归档入口列表。
    pub entries: Vec<JvmJarEntry>,
}

impl JvmJarPackage {
    /// 创建一个新的 `JAR` 产物模型。
    pub fn new(file_name: impl Into<String>) -> Self {
        Self { file_name: file_name.into(), main_class: None, entries: Vec::new() }
    }

    /// 追加一个归档入口。
    pub fn push_entry(&mut self, path: impl Into<String>, bytes: Vec<u8>) {
        self.entries.push(JvmJarEntry { path: path.into(), bytes });
    }

    /// 将 `ClassFile` 编码后放入 `JAR`。
    pub fn push_class(&mut self, class_file: &JvmClassFile) -> Result<(), JvmJarError> {
        let path = format!("{}.class", class_file.internal_name.trim_end_matches(".class"));
        self.push_entry(path, class_file.to_bytes()?);
        Ok(())
    }

    /// 从归档中读取指定类。
    pub fn read_class(&self, internal_name: &str) -> Result<Option<JvmClassFile>, JvmJarError> {
        let target_path = format!("{}.class", internal_name.trim_end_matches(".class"));
        let Some(entry) = self.entries.iter().find(|entry| entry.path == target_path)
        else {
            return Ok(None);
        };
        Ok(Some(JvmClassFile::from_bytes(&entry.bytes)?))
    }

    /// 遍历归档中所有 `.class` 入口，返回 `(路径, 字节切片)` 迭代器。
    pub fn iter_classes(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.entries
            .iter()
            .filter_map(|entry| entry.path.ends_with(".class").then(|| (entry.path.as_str(), entry.bytes.as_slice())))
    }

    /// 将 `JAR` 模型编码为 ZIP32 stored 归档。
    pub fn to_bytes(&self) -> Result<Vec<u8>, JvmJarError> {
        let mut members = Vec::new();
        if let Some(main_class) = &self.main_class {
            members.push((MANIFEST_PATH.to_string(), build_manifest(main_class).into_bytes()));
        }
        for entry in &self.entries {
            if entry.path.eq_ignore_ascii_case(MANIFEST_PATH) && self.main_class.is_some() {
                continue;
            }
            members.push((entry.path.clone(), entry.bytes.clone()));
        }
        write_stored_zip(&members)
    }

    /// 从二进制归档解码出 `JAR` 模型。容器索引用 `acorn-zip`。
    pub fn from_bytes(file_name: impl Into<String>, bytes: &[u8]) -> Result<Self, JvmJarError> {
        let owned = bytes.to_vec();
        let index = index_zip_bytes("jar", owned.clone())?;
        let mut package = Self::new(file_name);
        let budget = Budget::default();
        for member in &index.members {
            if member.path.ends_with('/') {
                continue;
            }
            let source = MemorySource::from_bytes("jar", owned.clone());
            let decoded = decode_member(member.path.clone(), source, member, &budget)?;
            let entry_bytes = read_source(&decoded)?;
            if member.path.eq_ignore_ascii_case(MANIFEST_PATH) {
                package.main_class = parse_manifest_main_class(&entry_bytes);
            }
            else {
                package.push_entry(member.path.clone(), entry_bytes);
            }
        }
        Ok(package)
    }
}

fn read_source(source: &MemorySource) -> Result<Vec<u8>, JvmJarError> {
    let length = source.length().unwrap_or(0) as usize;
    let mut buffer = vec![0u8; length];
    if length == 0 {
        return Ok(buffer);
    }
    match source.read_at(0, &mut buffer)? {
        ReadOutcome::Complete => Ok(buffer),
        ReadOutcome::Partial { bytes_read } => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        ReadOutcome::Eof => Ok(Vec::new()),
        ReadOutcome::Unavailable { .. } => Err(JvmJarError::Invalid("decoded member bytes are unavailable".into())),
    }
}

fn write_stored_zip(members: &[(String, Vec<u8>)]) -> Result<Vec<u8>, JvmJarError> {
    if members.len() > u16::MAX as usize {
        return Err(JvmJarError::Invalid("JAR member count exceeds ZIP32".into()));
    }
    let mut locals = Vec::new();
    let mut central = Vec::new();
    for (path, payload) in members {
        let name = path.as_bytes();
        if name.len() > u16::MAX as usize || payload.len() > u32::MAX as usize {
            return Err(JvmJarError::Invalid(format!("JAR member `{path}` exceeds ZIP32 stored limits")));
        }
        let crc = ieee_crc32(payload);
        let local_offset = locals.len() as u32;
        put32(&mut locals, LOCAL_SIGNATURE);
        put16(&mut locals, 20);
        put16(&mut locals, 1 << 11);
        put16(&mut locals, 0);
        put16(&mut locals, 0);
        put16(&mut locals, 0);
        put32(&mut locals, crc);
        put32(&mut locals, payload.len() as u32);
        put32(&mut locals, payload.len() as u32);
        put16(&mut locals, name.len() as u16);
        put16(&mut locals, 0);
        locals.extend_from_slice(name);
        locals.extend_from_slice(payload);

        put32(&mut central, CENTRAL_SIGNATURE);
        put16(&mut central, 20);
        put16(&mut central, 20);
        put16(&mut central, 1 << 11);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put32(&mut central, crc);
        put32(&mut central, payload.len() as u32);
        put32(&mut central, payload.len() as u32);
        put16(&mut central, name.len() as u16);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put32(&mut central, 0);
        put32(&mut central, local_offset);
        central.extend_from_slice(name);
    }
    let mut archive = locals;
    let cd_offset = archive.len() as u32;
    let cd_size = central.len() as u32;
    archive.extend(central);
    put32(&mut archive, EOCD_SIGNATURE);
    put16(&mut archive, 0);
    put16(&mut archive, 0);
    put16(&mut archive, members.len() as u16);
    put16(&mut archive, members.len() as u16);
    put32(&mut archive, cd_size);
    put32(&mut archive, cd_offset);
    put16(&mut archive, 0);
    Ok(archive)
}

fn put16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend(value.to_le_bytes());
}

fn put32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend(value.to_le_bytes());
}

fn ieee_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn build_manifest(main_class: &str) -> String {
    format!("Manifest-Version: 1.0\r\nMain-Class: {main_class}\r\n\r\n")
}

fn parse_manifest_main_class(bytes: &[u8]) -> Option<String> {
    let manifest = String::from_utf8_lossy(bytes);
    manifest.lines().find_map(|line| line.strip_prefix("Main-Class:").map(|value| value.trim().to_string()))
}
