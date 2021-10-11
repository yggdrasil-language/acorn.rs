//! Big-endian byte reader with boundedness checks.
//!
//! SFNT is entirely big-endian. Every getter panics only on programmer error
//! (e.g. calling [`Reader::new`] with an out-of-range bound); an unexpected end
//! of input returns [`ReadError`] rather than UB or a silent zero.

use std::fmt;

/// Outcome of a bounded read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// Requested range did not fit inside the active window.
    OutOfBounds {
        /// Absolute offset that was requested.
        offset: usize,
        /// Active window upper bound (exclusive).
        limit: usize,
        /// Number of bytes that were needed.
        need: usize,
    },
    /// Integer type overflow while promoting a width for internal use.
    WidthOverflow {
        /// Raw 32-bit width value that would overflow usize on this target.
        width: u32,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { offset, limit, need } => {
                write!(f, "read out of bounds: offset={offset} limit={limit} need={need}")
            }
            Self::WidthOverflow { width } => write!(f, "width overflow: {width} does not fit usize"),
        }
    }
}

impl std::error::Error for ReadError {}

/// A windowed big-endian reader over an `&[u8]`.
///
/// `Reader` holds a borrow of the underlying slice, a current cursor and an
/// exclusive upper bound. Every read that exceeds the bound returns
/// [`ReadError`]; the cursor is never advanced past a successful read.
///
/// Raw fields are `pub` so that sibling modules (`cmap`, `math`, `glyf`) can
/// carve fresh sub-readers or peek at arbitrary absolute offsets. The cursor
/// is still controlled by the reader's own methods.
#[derive(Clone, Copy)]
pub struct Reader<'a> {
    /// The underlying byte slice (shared borrow, outlives the reader).
    pub bytes: &'a [u8],
    /// Current cursor position (absolute into `bytes`).
    pub cursor: usize,
    /// Exclusive upper bound (absolute).
    pub limit: usize,
}

impl<'a> fmt::Debug for Reader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reader")
            .field("cursor", &self.cursor)
            .field("limit", &self.limit)
            .field("total", &self.bytes.len())
            .finish()
    }
}

impl<'a> Reader<'a> {
    /// Build a reader over the whole slice.
    #[inline]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0, limit: bytes.len() }
    }

    /// Build a reader constrained to `offset..offset+len`.
    ///
    /// Panics in debug if the requested window does not fit inside `bytes`.
    #[inline]
    pub fn at(bytes: &'a [u8], offset: usize, len: usize) -> Self {
        debug_assert!(offset.saturating_add(len) <= bytes.len());
        Self { bytes, cursor: offset, limit: offset + len }
    }

    /// Current absolute position.
    #[inline]
    pub fn position(&self) -> usize {
        self.cursor
    }

    /// Remaining bytes until the bound.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.cursor)
    }

    /// Seek to an absolute offset (still clamped by the reader's bound).
    #[inline]
    pub fn seek(&mut self, offset: usize) -> Result<(), ReadError> {
        if offset > self.limit {
            return Err(ReadError::OutOfBounds { offset, limit: self.limit, need: 0 });
        }
        self.cursor = offset;
        Ok(())
    }

    /// Advance by `delta` bytes from current position.
    #[inline]
    pub fn skip(&mut self, delta: usize) -> Result<(), ReadError> {
        let new = self.cursor.checked_add(delta).ok_or(ReadError::OutOfBounds {
            offset: self.cursor,
            limit: self.limit,
            need: delta,
        })?;
        if new > self.limit {
            return Err(ReadError::OutOfBounds { offset: self.cursor, limit: self.limit, need: delta });
        }
        self.cursor = new;
        Ok(())
    }

    /// Snapshot of `need` raw bytes starting at the current cursor.
    #[inline]
    pub fn bytes(&mut self, need: usize) -> Result<&'a [u8], ReadError> {
        let start = self.cursor;
        let end = start.checked_add(need).ok_or(ReadError::OutOfBounds { offset: start, limit: self.limit, need })?;
        if end > self.limit {
            return Err(ReadError::OutOfBounds { offset: start, limit: self.limit, need });
        }
        self.cursor = end;
        Ok(&self.bytes[start..end])
    }

    /// Consume the remainder of the current window.
    #[inline]
    pub fn rest(&mut self) -> &'a [u8] {
        let slice = &self.bytes[self.cursor..self.limit];
        self.cursor = self.limit;
        slice
    }

    /// Consume the remainder as a fresh reader anchored at the same cursor.
    #[inline]
    pub fn rest_reader(&self) -> Reader<'a> {
        Reader { bytes: self.bytes, cursor: self.cursor, limit: self.limit }
    }

    /// Peek one byte without advancing.
    #[inline]
    pub fn peek_u8(&self) -> Result<u8, ReadError> {
        if self.cursor >= self.limit {
            return Err(ReadError::OutOfBounds { offset: self.cursor, limit: self.limit, need: 1 });
        }
        Ok(self.bytes[self.cursor])
    }

    /// Read one `u8`.
    #[inline]
    pub fn read_u8(&mut self) -> Result<u8, ReadError> {
        if self.cursor >= self.limit {
            return Err(ReadError::OutOfBounds { offset: self.cursor, limit: self.limit, need: 1 });
        }
        let v = self.bytes[self.cursor];
        self.cursor += 1;
        Ok(v)
    }

    /// Read one big-endian `i8`.
    #[inline]
    pub fn read_i8(&mut self) -> Result<i8, ReadError> {
        self.read_u8().map(|v| v as i8)
    }

    /// Read one big-endian `u16`.
    #[inline]
    pub fn read_u16(&mut self) -> Result<u16, ReadError> {
        let bytes = self.bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// Read one big-endian `i16`.
    #[inline]
    pub fn read_i16(&mut self) -> Result<i16, ReadError> {
        let bytes = self.bytes(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// Read one big-endian `u32`.
    #[inline]
    pub fn read_u32(&mut self) -> Result<u32, ReadError> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read one big-endian `i32`.
    #[inline]
    pub fn read_i32(&mut self) -> Result<i32, ReadError> {
        let bytes = self.bytes(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a [`u64`] (two big-endian `u32` halves).
    #[inline]
    pub fn read_u64(&mut self) -> Result<u64, ReadError> {
        let bytes = self.bytes(8)?;
        Ok(u64::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]))
    }

    /// Read a [`i64`] (big-endian).
    #[inline]
    pub fn read_i64(&mut self) -> Result<i64, ReadError> {
        self.read_u64().map(|v| v as i64)
    }

    /// Read an SFNT `Fixed` (16.16 fixed-point).
    #[inline]
    pub fn read_fixed(&mut self) -> Result<f32, ReadError> {
        self.read_i32().map(|v| (v as f32) / 65536.0)
    }

    /// Read an SFNT `FWORD` (i16 interpreted as fixed-point).
    #[inline]
    pub fn read_fword(&mut self) -> Result<f32, ReadError> {
        self.read_i16().map(|v| v as f32)
    }

    /// Read an SFNT `UFWORD`.
    #[inline]
    pub fn read_ufword(&mut self) -> Result<f32, ReadError> {
        self.read_u16().map(|v| v as f32)
    }

    /// Read a [`Tag`] (four bytes).
    #[inline]
    pub fn read_tag(&mut self) -> Result<Tag, ReadError> {
        let bytes = self.bytes(4)?;
        Ok(Tag([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Convert a `u32` SFNT width value to `usize`, failing with a bounded
    /// error when it would overflow.
    #[inline]
    pub fn width_to_usize(width: u32) -> Result<usize, ReadError> {
        width.try_into().map_err(|_| ReadError::WidthOverflow { width })
    }
}

/// An SFNT four-byte tag (e.g. `b"name"`, `b"MATH"`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Tag(pub [u8; 4]);

impl Tag {
    /// Build from four bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Build from a `&[u8; 4]` literal.
    #[inline]
    pub const fn from_slice(bytes: &[u8; 4]) -> Self {
        Self(*bytes)
    }

    /// Return the inner bytes.
    #[inline]
    pub const fn as_bytes(self) -> [u8; 4] {
        self.0
    }

    /// ASCII-safe string representation. Non-printable bytes become `.`.
    #[inline]
    pub fn display(self) -> String {
        let dots: [u8; 4] = self.0.map(|b| if b.is_ascii_graphic() { b } else { b'.' });
        String::from_utf8_lossy(&dots).into_owned()
    }

    /// View as a `&str` if all bytes are ASCII.
    #[inline]
    pub fn as_str(self) -> Option<&'static str> {
        let s = std::str::from_utf8(&self.0).ok()?;
        Some(Box::leak(s.to_string().into_boxed_str()))
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag({})", self.display())
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl PartialEq<&[u8; 4]> for Tag {
    #[inline]
    fn eq(&self, other: &&[u8; 4]) -> bool {
        &self.0 == *other
    }
}
