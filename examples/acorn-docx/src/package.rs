use acorn_core::ParseBudget;
use acorn_source::{ByteSource, MemorySource};
use acorn_zip::{
    index_zip_bytes, read_member_payload, ZipArchiveIndex, ZipIndexError, ZipMember, ZipViewError,
};

/// Errors opening or reading an OPC package.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpcError {
    /// Underlying ZIP index failed.
    #[error("zip index error: {0}")]
    Zip(#[from] ZipIndexError),
    /// Member decode failed.
    #[error("zip view error: {0}")]
    View(#[from] ZipViewError),
    /// Part path was not found in the package.
    #[error("opc part not found: {0}")]
    PartNotFound(String),
    /// OPC XML could not be parsed.
    #[error("opc parse error: {0}")]
    Parse(String),
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
        let zip = index_zip_bytes("opc-bytes", read_all(&source)?)?;
        Ok(Self { source, zip })
    }

    /// ZIP index for the package.
    pub fn zip_index(&self) -> &ZipArchiveIndex {
        &self.zip
    }

    /// Returns a member for a normalized OPC part path.
    pub fn part(&self, path: &str) -> Option<&ZipMember> {
        let normalized = normalize_part_path(path);
        self.zip
            .members
            .iter()
            .find(|member| member.path == normalized)
    }

    /// Reads and decodes a package part into memory.
    pub fn read_part(&self, path: &str, budget: &ParseBudget) -> Result<Vec<u8>, OpcError> {
        let normalized = normalize_part_path(path);
        let member = self
            .part(&normalized)
            .ok_or_else(|| OpcError::PartNotFound(normalized))?;
        read_member_payload(self.source.clone(), member, budget).map_err(OpcError::from)
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
