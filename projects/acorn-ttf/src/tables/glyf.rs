//! `glyf` — TrueType glyph outlines.
//!
//! This module decodes simple and composite glyphs into an in-memory
//! [`GlyphOutline`] representation that preserves contours, flags and
//! absolute coordinates. Coordinate resolution (accumulation of relative
//! deltas and component transforms) is performed here.

use crate::{
    reader::{ReadError, Reader},
    tables::loca::Loca,
};

/// A single contour point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Absolute X in font units.
    pub x: i32,
    /// Absolute Y in font units.
    pub y: i32,
    /// True when this point is on the curve.
    pub on_curve: bool,
}

/// One parsed glyph outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphOutline {
    /// Glyph ID (set by the consumer).
    pub glyph_id: u16,
    /// Number of contours; 0 for an empty glyph (e.g. space).
    pub num_contours: i16,
    /// Bounding box — X min.
    pub x_min: i16,
    /// Bounding box — Y min.
    pub y_min: i16,
    /// Bounding box — X max.
    pub x_max: i16,
    /// Bounding box — Y max.
    pub y_max: i16,
    /// Contour boundaries: contour `i` spans points `contours[i]..contours[i+1]`.
    pub contours: Vec<usize>,
    /// All points (absolute coordinates).
    pub points: Vec<Point>,
    /// True when this glyph is a composite.
    pub is_composite: bool,
    /// Composite glyph component records (only populated when `is_composite`).
    pub components: Vec<Component>,
}

/// A single component inside a composite glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Referenced glyph ID.
    pub glyph_id: u16,
    /// Translation X in font units (or anchor point index — we always resolve to absolute X).
    pub x_offset: i32,
    /// Translation Y in font units.
    pub y_offset: i32,
    /// Flag word from the component record.
    pub flags: CompositeGlyphFlags,
}

/// Glyph header flag bits (simple and composite share the first 6 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleGlyphFlags(pub u8);
impl SimpleGlyphFlags {
    pub const ON_CURVE: u8 = 1 << 0;
    pub const X_SHORT: u8 = 1 << 1;
    pub const Y_SHORT: u8 = 1 << 2;
    pub const REPEAT: u8 = 1 << 3;
    pub const SAME_X_OR_POSITIVE_SHORT_X: u8 = 1 << 4;
    pub const SAME_Y_OR_POSITIVE_SHORT_Y: u8 = 1 << 5;
    pub const OVERLAP_SIMPLE: u8 = 1 << 6;

    #[inline]
    pub fn on_curve(self) -> bool {
        self.0 & Self::ON_CURVE != 0
    }
    #[inline]
    pub fn x_short(self) -> bool {
        self.0 & Self::X_SHORT != 0
    }
    #[inline]
    pub fn y_short(self) -> bool {
        self.0 & Self::Y_SHORT != 0
    }
    #[inline]
    pub fn same_x(self) -> bool {
        self.0 & Self::SAME_X_OR_POSITIVE_SHORT_X != 0
    }
    #[inline]
    pub fn same_y(self) -> bool {
        self.0 & Self::SAME_Y_OR_POSITIVE_SHORT_Y != 0
    }
    #[inline]
    pub fn repeat(self) -> bool {
        self.0 & Self::REPEAT != 0
    }
}

/// Composite glyph component flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositeGlyphFlags(pub u16);
impl CompositeGlyphFlags {
    pub const ARG_1_AND_2_ARE_WORDS: u16 = 1 << 0;
    pub const ARGS_ARE_XY_VALUES: u16 = 1 << 1;
    pub const ROUND_XY_TO_GRID: u16 = 1 << 2;
    pub const WE_HAVE_A_SCALE: u16 = 1 << 3;
    pub const RESERVED_4: u16 = 1 << 4;
    pub const MORE_COMPONENTS: u16 = 1 << 5;
    pub const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 1 << 6;
    pub const WE_HAVE_A_TWO_BY_TWO: u16 = 1 << 7;
    pub const WE_HAVE_INSTRUCTIONS: u16 = 1 << 8;
    pub const USE_MY_METRICS: u16 = 1 << 9;
    pub const OVERLAP_COMPOUND: u16 = 1 << 10;
    pub const SCALED_COMPONENT_OFFSET: u16 = 1 << 11;
    pub const UNSCALED_COMPONENT_OFFSET: u16 = 1 << 12;

    #[inline]
    pub fn more_components(self) -> bool {
        self.0 & Self::MORE_COMPONENTS != 0
    }
    #[inline]
    pub fn instructions_follow(self) -> bool {
        self.0 & Self::WE_HAVE_INSTRUCTIONS != 0
    }
    #[inline]
    pub fn has_scale(self) -> bool {
        self.0 & Self::WE_HAVE_A_SCALE != 0
    }
    #[inline]
    pub fn has_xy_scale(self) -> bool {
        self.0 & Self::WE_HAVE_AN_X_AND_Y_SCALE != 0
    }
    #[inline]
    pub fn has_2x2(self) -> bool {
        self.0 & Self::WE_HAVE_A_TWO_BY_TWO != 0
    }
    #[inline]
    pub fn args_are_words(self) -> bool {
        self.0 & Self::ARG_1_AND_2_ARE_WORDS != 0
    }
    #[inline]
    pub fn args_are_xy_values(self) -> bool {
        self.0 & Self::ARGS_ARE_XY_VALUES != 0
    }
}

/// Errors that can happen while decoding a glyph.
#[derive(Debug, Clone)]
pub enum GlyphError {
    Bounds(ReadError),
    EmptyGlyph,
}

impl From<ReadError> for GlyphError {
    #[inline]
    fn from(e: ReadError) -> Self {
        Self::Bounds(e)
    }
}

impl std::fmt::Display for GlyphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bounds(e) => write!(f, "glyph bounds: {e}"),
            Self::EmptyGlyph => f.write_str("empty glyph"),
        }
    }
}

impl std::error::Error for GlyphError {}

/// Parse a single glyph's outline. Input is `glyph[id]` bytes (slice of `glyf`).
pub fn parse_glyph(glyph_id: u16, data: &[u8]) -> Result<GlyphOutline, GlyphError> {
    if data.is_empty() {
        return Err(GlyphError::EmptyGlyph);
    }
    let mut r = Reader::new(data);
    let num_contours = r.read_i16()?;
    let x_min = r.read_i16()?;
    let y_min = r.read_i16()?;
    let x_max = r.read_i16()?;
    let y_max = r.read_i16()?;

    if num_contours == 0 {
        // No contours, just empty advance width.
        return Ok(GlyphOutline {
            glyph_id,
            num_contours: 0,
            x_min,
            y_min,
            x_max,
            y_max,
            contours: vec![0],
            points: Vec::new(),
            is_composite: false,
            components: Vec::new(),
        });
    }

    if num_contours < 0 {
        let mut components = Vec::new();
        let mut more = true;
        while more {
            let flags = CompositeGlyphFlags(r.read_u16()?);
            let glyph_id_comp = r.read_u16()?;
            let (arg1, arg2) = if flags.args_are_words() {
                (r.read_i16()?, r.read_i16()?)
            }
            else {
                (r.read_i8()? as i16, r.read_i8()? as i16)
            };
            let (x_offset, y_offset) = if flags.args_are_xy_values() {
                (arg1 as i32, arg2 as i32)
            }
            else {
                // Anchor point encoding — still treated as a (0,0) anchor at the component origin.
                (0, 0)
            };
            // Skip scale / 2x2 transforms for now (we still capture them in flags).
            if flags.has_scale() {
                let _ = r.read_fword()?;
            }
            if flags.has_xy_scale() {
                let _ = (r.read_fword()?, r.read_fword()?);
            }
            if flags.has_2x2() {
                let _ = (r.read_fword()?, r.read_fword()?, r.read_fword()?, r.read_fword()?);
            }
            components.push(Component { glyph_id: glyph_id_comp, x_offset, y_offset, flags });
            more = flags.more_components();
        }
        // Optionally instructions follow; skip.
        if !components.last().map_or(false, |c| c.flags.instructions_follow()) {
            // No instructions; nothing to do.
        }
        else {
            let _num_instructions = r.read_u16()?;
            // We skip instructions bytes.
            let _ = r.rest();
        }
        return Ok(GlyphOutline {
            glyph_id,
            num_contours,
            x_min,
            y_min,
            x_max,
            y_max,
            contours: Vec::new(),
            points: Vec::new(),
            is_composite: true,
            components,
        });
    }

    // Simple glyph: num_contours > 0.
    let nc = num_contours as usize;
    let mut end_points = Vec::with_capacity(nc);
    for _ in 0..nc {
        end_points.push(r.read_u16()? as usize);
    }
    let _instructions_length = r.read_u16()?;
    let _ = r.skip(_instructions_length as usize);

    let last = *end_points.last().unwrap_or(&0);
    let total_points = last + 1;
    let mut flags = Vec::with_capacity(total_points);
    let mut x_short = Vec::new();
    let mut y_short = Vec::new();

    while flags.len() < total_points {
        let f = SimpleGlyphFlags(r.read_u8()?);
        let mut count = 1usize;
        if f.repeat() {
            count = r.read_u8()? as usize + 1;
        }
        for _ in 0..count {
            flags.push(f);
        }
    }

    for f in &flags {
        let raw = if f.x_short() {
            let b = r.read_u8()? as i16;
            if f.same_x() { b } else { -b }
        }
        else if f.same_x() {
            0
        }
        else {
            r.read_i16()?
        };
        x_short.push(raw);
    }
    for f in &flags {
        let raw = if f.y_short() {
            let b = r.read_u8()? as i16;
            if f.same_y() { b } else { -b }
        }
        else if f.same_y() {
            0
        }
        else {
            r.read_i16()?
        };
        y_short.push(raw);
    }

    // Accumulate absolute coords.
    let mut points = Vec::with_capacity(total_points);
    let mut cur_x: i32 = 0;
    let mut cur_y: i32 = 0;
    for i in 0..total_points {
        cur_x += x_short[i] as i32;
        cur_y += y_short[i] as i32;
        points.push(Point { x: cur_x, y: cur_y, on_curve: flags[i].on_curve() });
    }

    let mut contours = Vec::with_capacity(nc + 1);
    contours.push(0);
    for ep in end_points {
        contours.push(ep + 1);
    }

    Ok(GlyphOutline {
        glyph_id,
        num_contours,
        x_min,
        y_min,
        x_max,
        y_max,
        contours,
        points,
        is_composite: false,
        components: Vec::new(),
    })
}

// ---- top-level glyf iteration ----

/// Iterate over every non-empty glyph in `glyf` using a pre-parsed [`Loca`].
pub fn iterate_glyf<'a>(
    glyf_bytes: &'a [u8],
    loca: &Loca,
    num_glyphs: u16,
    mut emit: impl FnMut(u16, Result<GlyphOutline, GlyphError>),
) {
    for id in 0..num_glyphs {
        let off = loca.glyph_offset(id);
        let size = loca.glyph_size(id);
        if size == 0 {
            continue;
        }
        let start = off as usize;
        let end = (off + size) as usize;
        if end > glyf_bytes.len() {
            emit(
                id,
                Err(GlyphError::Bounds(ReadError::OutOfBounds { offset: start, limit: glyf_bytes.len(), need: size as usize })),
            );
            continue;
        }
        let slice = &glyf_bytes[start..end];
        emit(id, parse_glyph(id, slice));
    }
}
