//! License + embedding-permission extraction from `name` + `OS/2`.
//!
//! Per OFL, SFS, GUST and other common licenses, we extract name records
//! and OS/2 `fsType` to synthesise a structured [`LicenseInfo`] that consumers
//! (notably VuTeX) can trust when deciding whether to embed a font.

use crate::{font::FontFace, tables::name::NameId};

/// Known license / embedding profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseKind {
    /// SIL Open Font License.
    OfL,
    /// GUST Font License (GUST e-foundry).
    Gust,
    /// GNU Free Documentation License.
    GFDL,
    /// Apache Software License (fonts from Google, e.g.).
    Apache,
    /// GPL (with font exception).
    Gpl,
    /// Proprietary / closed-source.
    Proprietary,
    /// Embedding disallowed.
    EmbeddingForbidden,
    /// Unknown.
    Unknown,
}

/// Structured embedding-permission + license facts.
#[derive(Debug, Clone, PartialEq)]
pub struct LicenseInfo {
    /// License kind.
    pub kind: LicenseKind,
    /// License text record (name ID 13).
    pub license_text: Option<String>,
    /// License URL record (name ID 14).
    pub license_url: Option<String>,
    /// Vendor / designer.
    pub vendor: Option<String>,
    /// `OS/2.fsType` raw bits.
    pub os2_fs_type: Option<u16>,
    /// True if the font allows embedding for installable / editable use.
    pub embedding_permitted: bool,
    /// True if embedding is restricted to preview-only.
    pub embedding_preview_only: bool,
}

impl LicenseInfo {
    /// Synthesise from a pre-loaded [`FontFace`].
    pub fn from_face(face: &mut FontFace) -> Self {
        let license_text = face.name().and_then(|n| n.get(NameId::LICENSE).map(|s| s.to_string()));
        let license_url = face.name().and_then(|n| n.get(NameId::LICENSE_URL).map(|s| s.to_string()));
        let vendor = face.name().and_then(|n| n.get(NameId::MANUFACTURER).map(|s| s.to_string()));
        let copyright = face.name().and_then(|n| n.get(NameId::COPYRIGHT).map(|s| s.to_string()));

        let os2_fs_type = face.os2().map(|o| o.fs_type);

        let kind =
            classify(license_text.as_deref(), license_url.as_deref(), vendor.as_deref(), os2_fs_type, copyright.as_deref());

        let embedding_permitted = os2_fs_type
            .map(|fs| {
                // bits 0-1 of fsType control embedding:
                // 00 = no restriction
                // 01 = restricted embedding
                // 10 = preview-only
                // 11 = print-only
                let restriction = fs & 0x0003;
                restriction != 0x0002 && restriction != 0x0003
            })
            .unwrap_or(true);
        let embedding_preview_only = os2_fs_type.map(|fs| (fs & 0x0003) == 0x0002).unwrap_or(false);

        Self { kind, license_text, license_url, vendor, os2_fs_type, embedding_permitted, embedding_preview_only }
    }
}

fn classify(
    text: Option<&str>,
    url: Option<&str>,
    vendor: Option<&str>,
    os2_fs_type: Option<u16>,
    copyright: Option<&str>,
) -> LicenseKind {
    if let Some(fs) = os2_fs_type {
        if (fs & 0x0003) == 0x0003 {
            return LicenseKind::EmbeddingForbidden;
        }
    }

    // Build one big haystack combining every source that could carry a license
    // hint. Lowercased, for case-insensitive substring match.
    let mut haystacks = Vec::new();
    for src in [text, url, vendor, copyright].into_iter().flatten() {
        haystacks.push(src.to_ascii_lowercase());
    }

    let any = |patterns: &[&str]| haystacks.iter().any(|h| patterns.iter().any(|p| h.contains(p)));

    if any(&["sil open font license", "open font license", "ofl.txt", "openfontlicense"]) {
        return LicenseKind::OfL;
    }
    if any(&["gust font license", "gust-font-license", "gust font"]) {
        return LicenseKind::Gust;
    }
    if any(&["gfdl", "free documentation license"]) {
        return LicenseKind::GFDL;
    }
    if any(&["apache software license", "apache license"]) {
        return LicenseKind::Apache;
    }
    if any(&["gnu public license", "gpl font exception"]) {
        return LicenseKind::Gpl;
    }
    if any(&["copyright", "all rights reserved"]) && !any(&["free", "open", "public domain", "license"]) {
        return LicenseKind::Proprietary;
    }

    LicenseKind::Unknown
}
