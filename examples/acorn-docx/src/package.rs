use acorn_core::BudgetTracker;
use acorn_source::{ByteSource, MemorySource};
use acorn_zip::{ZipArchiveIndex, ZipIndexError, ZipMember, ZipViewError, index_zip, read_member_payload};

/// Errors opening or reading an OPC package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpcError {
    /// Underlying ZIP index failed.
    Zip(ZipIndexError),
    /// Member decode failed.
    View(ZipViewError),
    /// Part path was not found in the package.
    PartNotFound(String),
    /// OPC XML could not be parsed.
    Parse(String),
}

impl std::fmt::Display for OpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip(error) => write!(f, "zip index error: {error}"),
            Self::View(error) => write!(f, "zip view error: {error}"),
            Self::PartNotFound(path) => write!(f, "opc part not found: {path}"),
            Self::Parse(message) => write!(f, "opc parse error: {message}"),
        }
    }
}

impl std::error::Error for OpcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Zip(error) => Some(error),
            Self::View(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ZipIndexError> for OpcError {
    fn from(error: ZipIndexError) -> Self {
        Self::Zip(error)
    }
}

impl From<ZipViewError> for OpcError {
    fn from(error: ZipViewError) -> Self {
        Self::View(error)
    }
}

/// Open Packaging Conventions package backed by a ZIP archive.
#[derive(Debug, Clone)]
pub struct OpcPackage {
    source: MemorySource,
    zip: ZipArchiveIndex,
}

impl OpcPackage {
    /// Indexes `bytes` as a DOCX OPC ZIP container.
    pub fn open(label: impl Into<String>, bytes: Vec<u8>) -> Result<Self, OpcError> {
        let source = MemorySource::from_bytes(label, bytes);
        // The index reads only the tail record, the directory, and member headers, so
        // opening a package no longer copies the whole archive.
        let zip = index_zip(&source)?;
        Ok(Self { source, zip })
    }

    /// ZIP index for the package.
    pub fn zip_index(&self) -> &ZipArchiveIndex {
        &self.zip
    }

    /// Returns a member for a normalized OPC part path.
    pub fn part(&self, path: &str) -> Option<&ZipMember> {
        let normalized = normalize_part_path(path);
        self.zip.members.iter().find(|member| member.path == normalized)
    }

    /// Reads and decodes a package part into memory.
    ///
    /// The part is settled against the caller's ledger, so every part of one document
    /// shares a single allowance.
    pub fn read_part(&self, path: &str, ledger: &mut BudgetTracker) -> Result<Vec<u8>, OpcError> {
        let normalized = normalize_part_path(path);
        let member = self.part(&normalized).ok_or_else(|| OpcError::PartNotFound(normalized))?;
        read_member_payload(self.source.clone(), member, ledger).map_err(OpcError::from)
    }

    /// Lists normalized part paths in central directory order.
    pub fn part_paths(&self) -> Vec<String> {
        self.zip.members.iter().map(|member| member.path.clone()).collect()
    }
}

/// Normalizes OPC part paths to forward slashes without a leading slash.
pub fn normalize_part_path(path: &str) -> String {
    let trimmed = path.trim().replace('\\', "/");
    trimmed.trim_start_matches('/').to_string()
}
