//! Tar entry facts shared by the reader, writer and gzip container.

use acorn_core::ByteRange;

/// Kind recorded by a tar typeflag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarEntryKind {
    /// Regular file (`0` or NUL typeflag, including the legacy contiguous flag).
    Regular,
    /// Hard link to an earlier member.
    HardLink,
    /// Symbolic link.
    Symlink,
    /// Character device.
    CharacterDevice,
    /// Block device.
    BlockDevice,
    /// Directory.
    Directory,
    /// FIFO.
    Fifo,
    /// Any typeflag this crate does not classify.
    Other(u8),
}

impl TarEntryKind {
    /// Typeflag written for this kind.
    pub const fn typeflag(self) -> u8 {
        match self {
            Self::Regular => b'0',
            Self::HardLink => b'1',
            Self::Symlink => b'2',
            Self::CharacterDevice => b'3',
            Self::BlockDevice => b'4',
            Self::Directory => b'5',
            Self::Fifo => b'6',
            Self::Other(flag) => flag,
        }
    }

    /// Classifies a header typeflag.
    pub const fn from_typeflag(flag: u8) -> Self {
        match flag {
            b'0' | 0 | b'7' => Self::Regular,
            b'1' => Self::HardLink,
            b'2' => Self::Symlink,
            b'3' => Self::CharacterDevice,
            b'4' => Self::BlockDevice,
            b'5' => Self::Directory,
            b'6' => Self::Fifo,
            other => Self::Other(other),
        }
    }

    /// Whether members of this kind carry payload bytes.
    pub const fn has_data(self) -> bool {
        matches!(self, Self::Regular)
    }

    /// Whether members of this kind may carry a link name.
    pub const fn has_link_name(self) -> bool {
        matches!(self, Self::HardLink | Self::Symlink)
    }
}

/// One indexed tar member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    /// Raw path bytes from the header, prefix, GNU long name or PAX override.
    ///
    /// The bytes are not decoded here, matching the archive containers' byte-level
    /// contract for names.
    pub path: Vec<u8>,
    /// Raw link target bytes for links, empty otherwise.
    pub link_name: Vec<u8>,
    /// Member kind.
    pub kind: TarEntryKind,
    /// Permission bits.
    pub mode: u32,
    /// Numeric owner id.
    pub uid: u32,
    /// Numeric group id.
    pub gid: u32,
    /// Declared payload size.
    pub size: u64,
    /// Modification time in Unix seconds.
    pub modified_unix: i64,
    /// Offset of the 512-byte header.
    pub header_offset: u64,
    /// Byte range of the payload in the tar byte space.
    pub data_range: ByteRange,
}

/// A tar member to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntryDraft {
    /// Raw path bytes.
    pub path: Vec<u8>,
    /// Payload bytes. Only regular members may carry data.
    pub data: Vec<u8>,
    /// Member kind.
    pub kind: TarEntryKind,
    /// Permission bits.
    pub mode: u32,
    /// Numeric owner id.
    pub uid: u32,
    /// Numeric group id.
    pub gid: u32,
    /// Modification time in Unix seconds.
    pub modified_unix: i64,
    /// Raw link target for link members.
    pub link_name: Vec<u8>,
}

impl TarEntryDraft {
    /// Creates a regular file with mode `0o644`.
    pub fn file(path: impl AsRef<[u8]>, data: impl Into<Vec<u8>>) -> Self {
        Self::new(path, data.into(), TarEntryKind::Regular, 0o644)
    }

    /// Creates a directory with mode `0o755`.
    pub fn directory(path: impl AsRef<[u8]>) -> Self {
        Self::new(path, Vec::new(), TarEntryKind::Directory, 0o755)
    }

    /// Creates a symbolic link with mode `0o777`.
    pub fn symlink(path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) -> Self {
        let mut entry = Self::new(path, Vec::new(), TarEntryKind::Symlink, 0o777);
        entry.link_name = target.as_ref().to_vec();
        entry
    }

    /// Creates a hard link with mode `0o644`.
    pub fn hard_link(path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) -> Self {
        let mut entry = Self::new(path, Vec::new(), TarEntryKind::HardLink, 0o644);
        entry.link_name = target.as_ref().to_vec();
        entry
    }

    fn new(path: impl AsRef<[u8]>, data: Vec<u8>, kind: TarEntryKind, mode: u32) -> Self {
        Self { path: path.as_ref().to_vec(), data, kind, mode, uid: 0, gid: 0, modified_unix: 0, link_name: Vec::new() }
    }
}

/// Indexed tar archive facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarArchive {
    /// Identity of the source that was indexed.
    pub identity: acorn_source::SourceIdentity,
    /// Indexed members in header order.
    pub entries: Vec<TarEntry>,
    /// Offset of the first end-of-archive zero block.
    pub end_of_archive: u64,
}
