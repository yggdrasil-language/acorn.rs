use acorn_core::ParseBudget;
use acorn_source::{ByteSource, MemorySource};
use acorn_zip::{
    index_zip_bytes, read_member_payload, ZipArchiveIndex, ZipIndexError, ZipMember, ZipViewError,
};

use crate::container::{parse_container_xml, ContainerRootFile};
use crate::opf::{parse_opf_xml, OpfDocument};

const CONTAINER_XML: &str = "META-INF/container.xml";

/// Errors opening or reading an EPUB OCF package.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OcfError {
    /// Underlying ZIP index failed.
    #[error("zip index error: {0}")]
    Zip(#[from] ZipIndexError),
    /// Member decode failed.
    #[error("zip view error: {0}")]
    View(#[from] ZipViewError),
    /// Package member was not found.
    #[error("ocf member not found: {0}")]
    MemberNotFound(String),
    /// Container or OPF XML could not be parsed.
    #[error("ocf parse error: {0}")]
    Parse(String),
}

/// EPUB OCF package backed by a ZIP archive.
#[derive(Debug, Clone)]
pub struct OcfPackage {
    source: MemorySource,
    zip: ZipArchiveIndex,
    rootfile: ContainerRootFile,
}

impl OcfPackage {
    /// Indexes `bytes` as an EPUB OCF ZIP container.
    pub fn open(label: impl Into<String>, bytes: Vec<u8>) -> Result<Self, OcfError> {
        let source = MemorySource::from_bytes(label, bytes);
        let zip = index_zip_bytes("ocf-bytes", read_all(&source)?)?;
        let budget = ParseBudget::default();
        let container_xml = read_member_bytes(&source, &zip, CONTAINER_XML, &budget)?;
        let rootfiles = parse_container_xml(&container_xml).map_err(OcfError::Parse)?;
        let rootfile = rootfiles
            .iter()
            .find(|root| root.media_type.contains("oebps-package"))
            .cloned()
            .or_else(|| rootfiles.first().cloned())
            .ok_or_else(|| OcfError::Parse("container.xml has no rootfile".into()))?;
        Ok(Self {
            source,
            zip,
            rootfile,
        })
    }

    /// Root OPF path from `META-INF/container.xml`.
    pub fn root_opf_path(&self) -> &str {
        &self.rootfile.full_path
    }

    /// ZIP index for the package.
    pub fn zip_index(&self) -> &ZipArchiveIndex {
        &self.zip
    }

    /// Returns a member for a normalized package path.
    pub fn member(&self, path: &str) -> Option<&ZipMember> {
        let normalized = normalize_member_path(path);
        self.zip
            .members
            .iter()
            .find(|member| member.path == normalized)
    }

    /// Reads and decodes a package member into memory.
    pub fn read_member(&self, path: &str, budget: &ParseBudget) -> Result<Vec<u8>, OcfError> {
        read_member_bytes(&self.source, &self.zip, path, budget)
    }

    /// Parses the root OPF document.
    pub fn read_opf(&self, budget: &ParseBudget) -> Result<OpfDocument, OcfError> {
        let xml = self.read_member(self.root_opf_path(), budget)?;
        parse_opf_xml(self.root_opf_path(), &xml).map_err(OcfError::Parse)
    }

    /// Lists normalized member paths in central directory order.
    pub fn member_paths(&self) -> Vec<String> {
        self.zip.members.iter().map(|member| member.path.clone()).collect()
    }

    /// Resolves a manifest `href` relative to the OPF directory.
    pub fn resolve_href(opf_path: &str, href: &str) -> String {
        let base = opf_path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
        let joined = if base.is_empty() {
            href.to_string()
        } else {
            format!("{}/{}", base, href)
        };
        normalize_member_path(&joined)
    }
}

/// Normalizes OCF member paths to forward slashes without a leading slash.
pub fn normalize_member_path(path: &str) -> String {
    let trimmed = path.trim().replace('\\', "/");
    trimmed.trim_start_matches('/').to_string()
}

fn read_member_bytes(
    source: &MemorySource,
    zip: &ZipArchiveIndex,
    path: &str,
    budget: &ParseBudget,
) -> Result<Vec<u8>, OcfError> {
    let normalized = normalize_member_path(path);
    let member = zip
        .members
        .iter()
        .find(|member| member.path == normalized)
        .ok_or_else(|| OcfError::MemberNotFound(normalized))?;
    read_member_payload(source.clone(), member, budget).map_err(OcfError::from)
}

fn read_all(source: &MemorySource) -> Result<Vec<u8>, ZipIndexError> {
    let length = source.length().ok_or(ZipIndexError::NotZip)? as usize;
    let mut buffer = vec![0u8; length];
    match source.read_at(0, &mut buffer) {
        Ok(acorn_source::ReadOutcome::Complete) => Ok(buffer),
        Ok(acorn_source::ReadOutcome::Partial { bytes_read }) => {
            buffer.truncate(bytes_read as usize);
            Ok(buffer)
        }
        Ok(acorn_source::ReadOutcome::Eof) => Err(ZipIndexError::NotZip),
        Ok(acorn_source::ReadOutcome::Unavailable { .. }) => Err(ZipIndexError::NotZip),
        Err(_) => Err(ZipIndexError::NotZip),
    }
}
