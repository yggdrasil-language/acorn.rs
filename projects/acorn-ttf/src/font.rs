//! High-level [`FontFace`] facade that aggregates parsed tables.
//!
//! This is the entry point consumers are expected to use for LatinModernMath-Regular.otf
//! and similar math fonts. The facade is intentionally non-magical — every sub-table
//! is parsed lazily on first access (or not at all if the table is missing).

use crate::{
    reader::Reader,
    sfnt::Face,
    tables::{
        cmap::CmapTable,
        glyf::GlyphOutline,
        head::HeadTable,
        hhea::HheaTable,
        hmtx::HmtxTable,
        loca::{Loca, LocaFormat, parse_loca},
        math::MathTable,
        maxp::MaxpTable,
        name::NameTable,
        os2::Os2Table,
        post::PostTable,
    },
};

/// Convenience: parse a [`FontFace`] from raw bytes.
pub fn load(data: &[u8]) -> Result<FontFace, crate::sfnt::ParseError> {
    let container = crate::sfnt::parse_font_validated(data)?;
    let face = container.first().ok_or(crate::sfnt::ParseError::TooSmall { need: 12, have: data.len() })?.clone();
    Ok(FontFace::from_face(face))
}

/// A single parsed font face with lazily-extracted tables.
pub struct FontFace {
    pub(crate) face: Face,
    pub(crate) name: Option<NameTable>,
    pub(crate) head: Option<HeadTable>,
    pub(crate) hhea: Option<HheaTable>,
    pub(crate) maxp: Option<MaxpTable>,
    pub(crate) os2: Option<Os2Table>,
    pub(crate) post: Option<PostTable>,
    pub(crate) hmtx: Option<HmtxTable>,
    pub(crate) cmap: Option<CmapTable>,
    pub(crate) math: Option<MathTable>,
    pub(crate) loca: Option<Loca>,
    pub(crate) is_opentype_cff: bool,
    pub(crate) is_truetype_glyf: bool,
}

impl std::fmt::Debug for FontFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontFace")
            .field("is_opentype_cff", &self.is_opentype_cff)
            .field("is_truetype_glyf", &self.is_truetype_glyf)
            .field("num_tables", &self.face.directory.len())
            .field("has_math", &self.math.is_some())
            .finish()
    }
}

impl FontFace {
    /// Convenience: parse a [`FontFace`] from raw bytes.
    pub fn load(data: &[u8]) -> Result<Self, crate::sfnt::ParseError> {
        let container = crate::sfnt::parse_font_validated(data)?;
        let face = container.first().ok_or(crate::sfnt::ParseError::TooSmall { need: 12, have: data.len() })?.clone();
        Ok(Self::from_face(face))
    }

    /// Wrap a pre-parsed [`Face`].
    pub fn from_face(face: Face) -> Self {
        let is_opentype_cff = face.header.is_opentype();
        let is_truetype_glyf = face.header.is_truetype();
        Self {
            face,
            name: None,
            head: None,
            hhea: None,
            maxp: None,
            os2: None,
            post: None,
            hmtx: None,
            cmap: None,
            math: None,
            loca: None,
            is_opentype_cff,
            is_truetype_glyf,
        }
    }

    /// Access the underlying [`Face`].
    #[inline]
    pub fn face(&self) -> &Face {
        &self.face
    }

    /// True if this face uses OTF + CFF outlines.
    #[inline]
    pub fn is_opentype_cff(&self) -> bool {
        self.is_opentype_cff
    }

    /// True if this face uses TrueType `glyf` outlines.
    #[inline]
    pub fn is_truetype_glyf(&self) -> bool {
        self.is_truetype_glyf
    }

    /// True if the font exposes a `MATH` table.
    #[inline]
    pub fn has_math(&self) -> bool {
        self.face.directory.get(b"MATH").is_some()
    }

    // --- standard table accessors ---

    pub fn name(&mut self) -> Option<&NameTable> {
        if self.name.is_none() {
            self.name = self.face.directory.get(b"name").and_then(|rec| {
                self.face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    NameTable::parse(&mut r).ok()
                })
            });
        }
        self.name.as_ref()
    }

    pub fn head(&mut self) -> Option<&HeadTable> {
        if self.head.is_none() {
            self.head = self.face.directory.get(b"head").and_then(|rec| {
                self.face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    HeadTable::parse(&mut r).ok()
                })
            });
        }
        self.head.as_ref()
    }

    pub fn hhea(&mut self) -> Option<&HheaTable> {
        if self.hhea.is_none() {
            self.hhea = self.face.directory.get(b"hhea").and_then(|rec| {
                self.face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    HheaTable::parse(&mut r).ok()
                })
            });
        }
        self.hhea.as_ref()
    }

    pub fn maxp(&mut self) -> Option<&MaxpTable> {
        if self.maxp.is_none() {
            self.maxp = self.face.directory.get(b"maxp").and_then(|rec| {
                self.face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    MaxpTable::parse(&mut r).ok()
                })
            });
        }
        self.maxp.as_ref()
    }

    pub fn os2(&mut self) -> Option<&Os2Table> {
        if self.os2.is_none() {
            self.os2 = self.face.directory.get(b"OS/2").and_then(|rec| {
                self.face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    Os2Table::parse(&mut r).ok()
                })
            });
        }
        self.os2.as_ref()
    }

    pub fn post(&mut self) -> Option<&PostTable> {
        if self.post.is_none() {
            self.post = self.face.directory.get(b"post").and_then(|rec| {
                self.face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    PostTable::parse(&mut r).ok()
                })
            });
        }
        self.post.as_ref()
    }

    pub fn cmap(&mut self) -> Option<&CmapTable> {
        if self.cmap.is_none() {
            self.cmap = self
                .face
                .directory
                .get(b"cmap")
                .and_then(|rec| self.face.table_data(rec).ok().and_then(|data| CmapTable::parse(data).ok()));
        }
        self.cmap.as_ref()
    }

    pub fn hmtx(&mut self) -> Option<&HmtxTable> {
        if self.hmtx.is_none() {
            let num_hmetrics = self.hhea()?.num_hmetrics;
            let num_glyphs = self.maxp()?.num_glyphs;
            let face = &self.face;
            self.hmtx = face.directory.get(b"hmtx").and_then(|rec| {
                face.table_data(rec).ok().and_then(|data| {
                    let mut r = Reader::new(data);
                    HmtxTable::parse(&mut r, num_glyphs, num_hmetrics).ok()
                })
            });
        }
        self.hmtx.as_ref()
    }

    pub fn math(&mut self) -> Option<&MathTable> {
        if self.math.is_none() {
            self.math = self
                .face
                .directory
                .get(b"MATH")
                .and_then(|rec| self.face.table_data(rec).ok().and_then(|data| MathTable::parse(data).ok()));
        }
        self.math.as_ref()
    }

    /// TrueType outline loca table (only useful when `is_truetype_glyf()`).
    pub fn loca(&mut self) -> Option<&Loca> {
        if self.loca.is_none() {
            let num_glyphs = self.maxp()?.num_glyphs;
            let long_loca = self.head()?.uses_long_loca();
            let face = &self.face;
            let loca_rec = face.directory.get(b"loca")?;
            let data = face.table_data(loca_rec).ok()?;
            let mut r = Reader::new(data);
            let fmt = if long_loca { LocaFormat::Long } else { LocaFormat::Short };
            self.loca = parse_loca(&mut r, num_glyphs, fmt).ok();
        }
        self.loca.as_ref()
    }

    /// Iterate over TrueType glyph outlines (empty when this is an OTF CFF font).
    pub fn glyf_outlines(&mut self) -> Vec<GlyphOutline> {
        let Some(loca) = self.loca().cloned()
        else {
            return Vec::new();
        };
        let maxp = self.maxp().copied().unwrap_or_default();
        let Some(glyf_rec) = self.face.directory.get(b"glyf")
        else {
            return Vec::new();
        };
        let Ok(glyf_data) = self.face.table_data(glyf_rec)
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        crate::tables::glyf::iterate_glyf(glyf_data, &loca, maxp.num_glyphs, |_id, res| {
            if let Ok(g) = res {
                out.push(g);
            }
        });
        out
    }

    // --- convenience strings ---

    pub fn family(&mut self) -> Option<String> {
        self.name()?
            .get(crate::tables::name::NameId::FAMILY)
            .map(|s| s.to_string())
            .or_else(|| self.name()?.get(crate::tables::name::NameId::TYPOGRAPHIC_FAMILY).map(|s| s.to_string()))
    }

    pub fn subfamily(&mut self) -> Option<String> {
        self.name()?.get(crate::tables::name::NameId::SUBFAMILY).map(|s| s.to_string())
    }

    pub fn full_name(&mut self) -> Option<String> {
        self.name()?.get(crate::tables::name::NameId::FULL).map(|s| s.to_string())
    }

    pub fn postscript_name(&mut self) -> Option<String> {
        self.name()?.get(crate::tables::name::NameId::POSTSCRIPT).map(|s| s.to_string())
    }

    pub fn license(&mut self) -> Option<String> {
        self.name()?.get(crate::tables::name::NameId::LICENSE).map(|s| s.to_string())
    }

    pub fn license_url(&mut self) -> Option<String> {
        self.name()?.get(crate::tables::name::NameId::LICENSE_URL).map(|s| s.to_string())
    }
}

// Provide a way to look up arbitrary tags too (advanced access).
impl FontFace {
    pub fn table_bytes(&self, tag_bytes: &[u8; 4]) -> Option<&[u8]> {
        let rec = self.face.directory.get(tag_bytes)?;
        self.face.table_data(rec).ok()
    }

    pub fn table_record(&self, tag_bytes: &[u8; 4]) -> Option<&crate::sfnt::TableRecord> {
        self.face.directory.get(tag_bytes)
    }
}
