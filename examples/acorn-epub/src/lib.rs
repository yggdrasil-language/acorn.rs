#![warn(missing_docs)]
//! EPUB OCF package access for Acorn document pipelines.

mod container;
mod opf;
mod package;
mod xml;

pub use container::ContainerRootFile;
pub use opf::{ManifestItem, OpfDocument, OpfMetadata, SpineItem};
pub use package::{normalize_member_path, OcfError, OcfPackage};
