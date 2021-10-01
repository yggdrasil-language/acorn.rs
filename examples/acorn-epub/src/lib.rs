#![warn(missing_docs)]
//! EPUB OCF package access for Acorn document pipelines.

mod container;
mod opf;
mod package;
mod xml;

pub use container::ContainerRootFile;
pub use opf::{ManifestItem, OpfDocument, OpfMetadata, SpineItem, parse_opf_xml};
pub use package::{OcfError, OcfPackage, normalize_member_path};
pub use xml::{attribute_value, document_root, element_is, elements_by_local_name, parse_xml_bytes};
