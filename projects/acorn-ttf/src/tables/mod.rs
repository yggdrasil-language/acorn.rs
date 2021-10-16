//! Per-table parsers.
//!
//! All parsers operate on a byte slice that is guaranteed to be the exact
//! payload of the named table (the [`crate::sfnt::Face::table_data`] method
//! slices this for us). They return structured facts and never allocate
//! outside the requested scope unless the table itself requires a lookup
//! allocation (e.g. `cmap` char→glyph map).

pub mod cff;
pub mod cff2;
pub mod cmap;
pub mod glyf;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod loca;
pub mod math;
pub mod maxp;
pub mod name;
pub mod os2;
pub mod post;

pub use cff::{CffIndex, CffInfo, parse_cff_header};
pub use cff2::{Cff2Info, parse_cff2_header};
pub use cmap::{CmapSubtable, CmapSubtableData, CmapTable, EncodingRecord};
pub use glyf::{CompositeGlyphFlags, GlyphOutline, SimpleGlyphFlags};
pub use head::{HeadTable, MacStyle};
pub use hhea::HheaTable;
pub use hmtx::{HmtxTable, LongHorMetric};
pub use loca::{LocaFormat, parse_loca};
pub use math::{
    GlyphAssembly, MathConstants, MathGlyphConstruction, MathGlyphInfo, MathKernTable, MathTable, MathVariantRecord,
    MathVariants, PartRecord,
};
pub use maxp::MaxpTable;
pub use name::{NameId, NameRecord, NameTable};
pub use os2::Os2Table;
pub use post::PostTable;
