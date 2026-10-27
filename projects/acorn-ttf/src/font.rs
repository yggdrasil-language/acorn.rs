//! High-level [`FontFace`] facade that aggregates parsed tables.
//!
//! This is the entry point consumers are expected to use for LatinModernMath-Regular.otf
//! and similar math fonts. The facade is intentionally non-magical — every sub-table
//! is parsed lazily on first access (or not at all if the table is missing).

use crate::{
    outline::{BBox, OutlineBuilder},
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
            let num_glyphs = self.maxp().map(|maxp| maxp.num_glyphs);
            self.math = self.face.directory.get(b"MATH").and_then(|rec| {
                self.face
                    .table_data(rec)
                    .ok()
                    .and_then(|data| MathTable::parse_with_num_glyphs(data, num_glyphs).ok())
            });
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

    // ---- high-level glyph convenience API (ttf-parser-compatible surface) ----

    /// Shortcut for `self.head()?.units_per_em`.
    pub fn units_per_em(&mut self) -> u16 {
        self.head().map(|h| h.units_per_em).unwrap_or(1000)
    }

    /// Look up a Unicode scalar → glyph ID via `cmap`.
    pub fn glyph_index(&mut self, ch: char) -> Option<u16> {
        self.cmap()?.lookup(ch as u32)
    }

    /// Horizontal advance width for the given glyph.
    pub fn glyph_hor_advance(&mut self, glyph_id: u16) -> Option<u16> {
        self.hmtx()?.get(glyph_id).map(|m| m.advance_width)
    }

    /// Per-glyph bounding box in font units.
    ///
    /// Returns `None` when the outline kind cannot supply a per-glyph box
    /// (e.g. OTF CFF without the CFF bbox feature). In that case callers can
    /// fall back to a synthetic estimate.
    pub fn glyph_bounding_box(&mut self, glyph_id: u16) -> Option<BBox> {
        if self.is_truetype_glyf {
            let maxp = self.maxp()?.num_glyphs;
            if glyph_id >= maxp {
                return None;
            }
            let loca = self.loca().cloned()?;
            let Some(glyf_rec) = self.face.directory.get(b"glyf")
            else {
                return None;
            };
            let Ok(glyf_data) = self.face.table_data(glyf_rec)
            else {
                return None;
            };
            let off = loca.glyph_offset(glyph_id);
            let size = loca.glyph_size(glyph_id);
            if size == 0 {
                return None;
            }
            let start = off as usize;
            let end = (off + size) as usize;
            if end > glyf_data.len() {
                return None;
            }
            let slice = &glyf_data[start..end];
            let outline = crate::tables::glyf::parse_glyph(glyph_id, slice).ok()?;
            if outline.num_contours <= 0 {
                return None;
            }
            Some(BBox { x_min: outline.x_min, y_min: outline.y_min, x_max: outline.x_max, y_max: outline.y_max })
        }
        else {
            self.cff_glyph_bounding_box(glyph_id)
        }
    }

    /// Emit glyph outline segments into the given [`OutlineBuilder`].
    ///
    /// Returns `Some(())` when an outline was successfully decoded. `None`
    /// when the glyph has no outline (e.g. empty space glyph) or the
    /// outline kind is not available in this build.
    pub fn outline_glyph(&mut self, glyph_id: u16, builder: &mut dyn OutlineBuilder) -> Option<()> {
        if self.is_truetype_glyf { self.outline_glyf(glyph_id, builder) } else { self.outline_cff(glyph_id, builder) }
    }

    // ---- TrueType glyf → OutlineBuilder ----

    fn outline_glyf(&mut self, glyph_id: u16, builder: &mut dyn OutlineBuilder) -> Option<()> {
        let maxp = self.maxp()?.num_glyphs;
        if glyph_id >= maxp {
            return None;
        }
        let loca = self.loca().cloned()?;
        let Some(glyf_rec) = self.face.directory.get(b"glyf")
        else {
            return None;
        };
        let Ok(glyf_data) = self.face.table_data(glyf_rec)
        else {
            return None;
        };
        let off = loca.glyph_offset(glyph_id);
        let size = loca.glyph_size(glyph_id);
        if size == 0 {
            return None;
        }
        let start = off as usize;
        let end = (off + size) as usize;
        if end > glyf_data.len() {
            return None;
        }
        let slice = &glyf_data[start..end];
        let outline = crate::tables::glyf::parse_glyph(glyph_id, slice).ok()?;
        if outline.num_contours <= 0 || outline.points.is_empty() {
            return None;
        }
        emit_glyf_outline(&outline, builder);
        Some(())
    }

    /// OTF CFF path — currently returns `None` unless the `outline-cff`
    /// feature is enabled, in which case it delegates to `ttf-parser`.
    fn outline_cff(&mut self, _glyph_id: u16, _builder: &mut dyn OutlineBuilder) -> Option<()> {
        #[cfg(feature = "outline-cff")]
        {
            self.outline_cff_via_ttf_parser(_glyph_id, _builder)
        }
        #[cfg(not(feature = "outline-cff"))]
        {
            None
        }
    }

    /// OTF CFF bbox — currently returns `None` unless the `outline-cff`
    /// feature is enabled.
    fn cff_glyph_bounding_box(&mut self, _glyph_id: u16) -> Option<BBox> {
        #[cfg(feature = "outline-cff")]
        {
            self.cff_bbox_via_ttf_parser(_glyph_id)
        }
        #[cfg(not(feature = "outline-cff"))]
        {
            None
        }
    }

    #[cfg(feature = "outline-cff")]
    fn outline_cff_via_ttf_parser(&self, glyph_id: u16, builder: &mut dyn OutlineBuilder) -> Option<()> {
        use ttf_parser::{Face as TtFace, OutlineBuilder as TtOutlineBuilder};

        // We re-wrap acorn's builder in an adapter that satisfies ttf-parser's
        // OutlineBuilder trait — the two traits have identical shape.
        struct Adapter<'a>(&'a mut dyn OutlineBuilder);
        impl TtOutlineBuilder for Adapter<'_> {
            fn move_to(&mut self, x: f32, y: f32) {
                self.0.move_to(x, y);
            }
            fn line_to(&mut self, x: f32, y: f32) {
                self.0.line_to(x, y);
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                self.0.quad_to(x1, y1, x, y);
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                self.0.curve_to(x1, y1, x2, y2, x, y);
            }
            fn close(&mut self) {
                self.0.close();
            }
        }

        let tt_face = TtFace::parse(&self.face.bytes, 0).ok()?;
        let gid = ttf_parser::GlyphId(glyph_id);
        tt_face.outline_glyph(gid, &mut Adapter(builder))?;
        Some(())
    }

    #[cfg(feature = "outline-cff")]
    fn cff_bbox_via_ttf_parser(&self, glyph_id: u16) -> Option<BBox> {
        use ttf_parser::Face as TtFace;
        let tt_face = TtFace::parse(&self.face.bytes, 0).ok()?;
        let gid = ttf_parser::GlyphId(glyph_id);
        let rect = tt_face.glyph_bounding_box(gid)?;
        Some(BBox { x_min: rect.x_min, y_min: rect.y_min, x_max: rect.x_max, y_max: rect.y_max })
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

// ---- TrueType glyph → OutlineBuilder ----

/// Emit one TrueType glyph outline as quadratic Bezier segments.
///
/// TrueType stores contours as a ring of `Point { on_curve, x, y }`. Any two
/// consecutive off-curve points imply an on-curve midpoint between them. The
/// ring wraps around, so we also connect the last point back to the first.
///
/// Composite glyphs (num_contours < 0) are not supported by this routine —
/// callers should fall back to a different backend when `is_composite` is true.
fn emit_glyf_outline(outline: &GlyphOutline, builder: &mut dyn OutlineBuilder) {
    if outline.is_composite || outline.points.is_empty() {
        return;
    }
    let contours = &outline.contours;
    let points = &outline.points;
    let num_contours = outline.num_contours as usize;

    for c in 0..num_contours {
        let start_idx = contours[c];
        let end_idx = contours[c + 1]; // exclusive
        let n = end_idx - start_idx;
        if n == 0 {
            continue;
        }
        let mut i = 0usize;
        // Step 1: emit contour start — find the first on-curve point, or
        // synthesize an implicit midpoint if the contour begins off-curve.
        let first_on = find_next_on(points, start_idx, end_idx);
        let start_pt = if first_on >= n {
            // All points are off-curve — degenerate; skip.
            continue;
        }
        else {
            let gidx = start_idx + first_on;
            let p = points[gidx];
            builder.move_to(p.x as f32, p.y as f32);
            first_on + 1
        };
        i = start_pt;
        let mut prev = start_pt;

        // Step 2: walk the ring, emitting segments.
        let mut emitted_start = false;
        while emitted_start || i != start_pt {
            if !emitted_start {
                emitted_start = true;
                // We already move_to'd the first on-curve — continue.
            }
            let cur_idx = start_idx + (i % n);
            let cur = points[cur_idx];
            let prev_pt = points[start_idx + (prev % n)];

            if cur.on_curve {
                // Previous point was off-curve (or this is start); emit quad.
                if prev_pt.on_curve {
                    builder.line_to(cur.x as f32, cur.y as f32);
                }
                else {
                    builder.quad_to(prev_pt.x as f32, prev_pt.y as f32, cur.x as f32, cur.y as f32);
                }
            }
            else {
                // Current is off-curve; look ahead to next point.
                let next_idx = start_idx + ((i + 1) % n);
                let next = points[next_idx];
                if next.on_curve {
                    // off-curve → on-curve: emit quad from prev cur → next
                    if prev_pt.on_curve {
                        builder.quad_to(cur.x as f32, cur.y as f32, next.x as f32, next.y as f32);
                    }
                    else {
                        // prev off-curve → cur off-curve → next on-curve:
                        // synthetic midpoint between prev and cur, then cur→next quad.
                        let mid_x = (prev_pt.x + cur.x) as f32 * 0.5;
                        let mid_y = (prev_pt.y + cur.y) as f32 * 0.5;
                        builder.quad_to(cur.x as f32, cur.y as f32, next.x as f32, next.y as f32);
                        // Note: the previous quad would have been emitted at the prev step.
                    }
                }
                else {
                    // prev off-curve, cur off-curve, next off-curve (or the on-curve comes later).
                    // We just skip cur; the midpoint between prev and next is the implicit on-curve.
                }
            }
            prev = i;
            i = (i + 1) % n;
            if i == start_pt {
                break;
            }
        }

        builder.close();
    }
}

fn find_next_on(points: &[crate::tables::glyf::Point], start: usize, end: usize) -> usize {
    let n = end - start;
    for off in 0..n {
        let idx = start + off;
        if idx < points.len() && points[idx].on_curve {
            return off;
        }
    }
    n
}
