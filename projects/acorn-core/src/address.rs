use thiserror::Error;

/// Identifier for an address space within one parse session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddressSpaceId(pub u32);

/// Well-known address space kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AddressSpaceKind {
    /// Stable byte offsets of the original input.
    File,
    /// Bounded sub-view, such as an archive member or partition.
    View,
    /// Decompressed, decoded, or transformed logical bytes.
    Decoded,
    /// Format-declared virtual addresses used only for reference resolution.
    Virtual,
    /// Object numbering assigned by a format directory or index.
    Object,
}

/// A location inside a specific address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Address {
    /// Address space containing the offset.
    pub space: AddressSpaceId,
    /// Byte offset from the start of the space.
    pub offset: u64,
}

/// A half-open byte interval `[start, start + len)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ByteRange {
    /// Inclusive start offset.
    pub start: u64,
    /// Length in bytes.
    pub len: u64,
}

/// Errors produced by checked range arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RangeError {
    /// `start + len` overflowed `u64`.
    #[error("byte range overflow")]
    Overflow,
    /// The range end exceeds the declared space length.
    #[error("byte range exceeds space length")]
    OutOfBounds,
    /// `len` is zero where a non-empty range is required.
    #[error("empty byte range")]
    Empty,
}

impl ByteRange {
    /// Creates a new range after validating `start + len`.
    pub fn new(start: u64, len: u64) -> Result<Self, RangeError> {
        if len == 0 {
            return Err(RangeError::Empty);
        }
        let end = start.checked_add(len).ok_or(RangeError::Overflow)?;
        if end < start {
            return Err(RangeError::Overflow);
        }
        Ok(Self { start, len })
    }

    /// Returns the exclusive end offset, if it can be computed.
    pub fn end(self) -> Result<u64, RangeError> {
        self.start.checked_add(self.len).ok_or(RangeError::Overflow)
    }

    /// Returns whether `self` is fully contained in `[0, space_len)`.
    pub fn fits_in(self, space_len: u64) -> Result<(), RangeError> {
        let end = self.end()?;
        if end > space_len {
            return Err(RangeError::OutOfBounds);
        }
        Ok(())
    }

    /// Returns whether two ranges overlap in the same address space.
    pub fn overlaps(self, other: Self) -> Result<bool, RangeError> {
        let self_end = self.end()?;
        let other_end = other.end()?;
        Ok(self.start < other_end && other.start < self_end)
    }
}

impl Address {
    /// Creates a new address.
    pub const fn new(space: AddressSpaceId, offset: u64) -> Self {
        Self { space, offset }
    }
}
