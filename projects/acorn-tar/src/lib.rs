#![warn(missing_docs)]
//! Tar container indexing, writing and tar-gzip composition for Acorn.

mod format;
mod model;
mod read;
mod targz;
mod write;

pub use model::{TarArchive, TarEntry, TarEntryDraft, TarEntryKind};
pub use read::{TarIndexError, TarReadError, index_tar, index_tar_bytes, read_member_payload};
pub use targz::{TarGz, TarGzArchive, TarGzError};
pub use write::{TarWriteError, TarWriter, write_tar};
