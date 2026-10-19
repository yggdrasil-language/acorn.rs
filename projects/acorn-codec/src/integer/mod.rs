//! Integer domain transforms.

mod leb128;
mod zigzag;

pub use self::{
    leb128::{Leb128, Leb128Profile},
    zigzag::ZigZag,
};
