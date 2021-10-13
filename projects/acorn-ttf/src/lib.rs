//! `acorn-ttf` — SFNT font facts for the Acorn ecosystem.
//!
//! This crate parses TrueType and OpenType font files (single `.ttf` / `.otf`
//! or TTC collections) and exposes raw **facts**: table directories, standard
//! metrics, the `MATH` table, TrueType `glyf` outlines and OTF CFF outlines.
//!
//! It does **not** render glyphs, build SVG paths, select fonts, apply
//! spacing rules or compress/decompress WOFF. Those jobs belong to consumers
//! (notably VuTeX and other formula-layout projects).
//!
//! All parsing is bounded: truncated input, unknown offsets, and bad
//! checksums surface as structured results rather than panics. Unknown tables
//! are preserved in the directory and annotated rather than rejected.

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod outline;
pub mod reader;
pub mod sfnt;
pub mod tables;

// Top-level re-exports for the most common entry points.
pub use outline::{BBox, OutlineBuilder};
pub use reader::{ReadError, Reader, Tag};
pub use sfnt::{
    ChecksumStatus, Face, FontContainer, OffsetTable, ParseError, TableDirectory, TableRecord, file_checksum, magic,
    parse_font, parse_font_validated, table_checksum, validate_checksums,
};

// Diagnostic helpers bridge to `acorn-core`.
pub mod diag {
    //! Diagnostic constructors prefixed with `acorn.ttf.*` wire codes.
    use acorn_core::{
        diagnostic as acd,
        diagnostic::{DiagnosticCode, DiagnosticSeverity},
    };

    /// Stable dotted identifier per ttf-specific diagnostic.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TtfCode {
        /// Table checksum mismatch.
        ChecksumMismatch,
        /// Unknown SFNT version field.
        UnknownVersion,
        /// A required standard table is missing.
        MissingStandardTable,
        /// Out-of-bounds offset inside a table.
        TableOffsetOutOfRange,
        /// Malformed `cmap` subtable.
        CmapMalformed,
        /// Malformed `maxp` table.
        MaxpMalformed,
        /// Unsupported MATH version.
        MathUnsupportedVersion,
        /// MATH substructure truncated.
        MathTruncated,
        /// Glyph ID outside font range.
        GlyphOutOfRange,
    }

    impl TtfCode {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::ChecksumMismatch => "acorn.ttf.checksum",
                Self::UnknownVersion => "acorn.ttf.unknown-version",
                Self::MissingStandardTable => "acorn.ttf.missing-standard-table",
                Self::TableOffsetOutOfRange => "acorn.ttf.offset-out-of-range",
                Self::CmapMalformed => "acorn.ttf.cmap-malformed",
                Self::MaxpMalformed => "acorn.ttf.maxp-malformed",
                Self::MathUnsupportedVersion => "acorn.ttf.math-unsupported-version",
                Self::MathTruncated => "acorn.ttf.math-truncated",
                Self::GlyphOutOfRange => "acorn.ttf.glyph-out-of-range",
            }
        }
    }

    /// Build an `acorn.ttf.*` diagnostic at the given severity.
    pub fn ttf(code: TtfCode, severity: DiagnosticSeverity, message: impl Into<String>) -> acd::Diagnostic {
        use acorn_core::diagnostic::*;
        let wire = WireDiagnosticCode::new(code.as_str());
        let origin = DiagnosticOrigin::new("acorn-ttf", "table");
        Diagnostic::new(wire, severity, origin, Message::new(code.as_str()).with_fallback(message.into()))
    }

    /// Convenience: wrap a `ReadError` into an `acorn.layout.truncated`.
    pub fn bounded(e: crate::reader::ReadError) -> acd::Diagnostic {
        acd::error(DiagnosticCode::Truncated, e.to_string())
    }
}

/// Pull in a minimal license-aware analysis.
pub mod license;

/// Font-level facade that collects parsed tables off a [`Face`].
pub mod font;

// Re-export face-level convenience.
pub use font::FontFace;
