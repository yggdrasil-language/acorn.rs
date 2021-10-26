//! Tar header parsing and emission helpers.

/// Tar block size in bytes.
pub(crate) const BLOCK_SIZE: u64 = 512;

/// Offset of the checksum field in a tar header.
const CHECKSUM_OFFSET: usize = 148;
/// End offset of the checksum field.
const CHECKSUM_END: usize = 156;
/// Offset of the typeflag byte.
const TYPEFLAG_OFFSET: usize = 156;
/// Offset of the link-name field.
const LINK_NAME_OFFSET: usize = 157;
/// Offset of the magic field.
const MAGIC_OFFSET: usize = 257;
/// Offset of the prefix field.
const PREFIX_OFFSET: usize = 345;
/// Length of the prefix field.
const PREFIX_LENGTH: usize = 155;

/// Header fields read from a 512-byte block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    pub name: Vec<u8>,
    pub prefix: Vec<u8>,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub modified_unix: i64,
    pub typeflag: u8,
    pub link_name: Vec<u8>,
}

impl Header {
    /// Full path from the ustar name and prefix fields.
    pub fn path(&self) -> Vec<u8> {
        if self.prefix.is_empty() {
            return self.name.clone();
        }
        let mut path = self.prefix.clone();
        path.push(b'/');
        path.extend_from_slice(&self.name);
        path
    }
}

/// Header validation failure without an archive offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeaderError {
    ChecksumMismatch { expected: u32, actual: u32 },
    InvalidField(&'static str),
}

/// Parses a 512-byte tar header and validates its checksum.
pub(crate) fn parse_header(block: &[u8]) -> Result<Header, HeaderError> {
    let expected = parse_checksum(&block[CHECKSUM_OFFSET..CHECKSUM_END]).ok_or(HeaderError::InvalidField("checksum"))?;
    let actual = header_checksum(block);
    if expected != actual {
        return Err(HeaderError::ChecksumMismatch { expected, actual });
    }

    let size = parse_number(&block[124..136]).ok_or(HeaderError::InvalidField("size"))?;
    let size = u64::try_from(size).map_err(|_| HeaderError::InvalidField("size"))?;
    let mode = parse_number(&block[100..108]).ok_or(HeaderError::InvalidField("mode"))?;
    let uid = parse_number(&block[108..116]).ok_or(HeaderError::InvalidField("uid"))?;
    let gid = parse_number(&block[116..124]).ok_or(HeaderError::InvalidField("gid"))?;
    let modified_unix = parse_number(&block[136..148]).ok_or(HeaderError::InvalidField("mtime"))?;
    let magic = &block[MAGIC_OFFSET..MAGIC_OFFSET + 6];
    let has_prefix = magic == b"ustar\0" || magic == b"ustar ";

    Ok(Header {
        name: c_string(&block[..100]),
        prefix: if has_prefix { c_string(&block[PREFIX_OFFSET..PREFIX_OFFSET + PREFIX_LENGTH]) } else { Vec::new() },
        mode: u32::try_from(mode).map_err(|_| HeaderError::InvalidField("mode"))?,
        uid: u32::try_from(uid).map_err(|_| HeaderError::InvalidField("uid"))?,
        gid: u32::try_from(gid).map_err(|_| HeaderError::InvalidField("gid"))?,
        size,
        modified_unix,
        typeflag: block[TYPEFLAG_OFFSET],
        link_name: c_string(&block[LINK_NAME_OFFSET..LINK_NAME_OFFSET + 100]),
    })
}

/// Builds a ustar header with the given path fields.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_header(
    name: &[u8],
    prefix: &[u8],
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
    modified_unix: i64,
    typeflag: u8,
    link_name: &[u8],
) -> [u8; 512] {
    let mut block = [0u8; 512];
    block[..name.len()].copy_from_slice(name);
    block[CHECKSUM_OFFSET..CHECKSUM_END].fill(b' ');
    block[TYPEFLAG_OFFSET] = typeflag;
    block[LINK_NAME_OFFSET..LINK_NAME_OFFSET + link_name.len()].copy_from_slice(link_name);
    block[MAGIC_OFFSET..MAGIC_OFFSET + 6].copy_from_slice(b"ustar\0");
    block[MAGIC_OFFSET + 6..MAGIC_OFFSET + 8].copy_from_slice(b"00");
    block[PREFIX_OFFSET..PREFIX_OFFSET + prefix.len()].copy_from_slice(prefix);

    write_number(&mut block[100..108], i64::from(mode));
    write_number(&mut block[108..116], i64::from(uid));
    write_number(&mut block[116..124], i64::from(gid));
    write_number(&mut block[124..136], size as i64);
    write_number(&mut block[136..148], modified_unix);

    let checksum = header_checksum(&block);
    let text = format!("{checksum:06o}\0 ");
    block[CHECKSUM_OFFSET..CHECKSUM_END].copy_from_slice(&text.as_bytes()[..8]);
    block
}

/// Splits a path into a ustar name and prefix when both fields can hold it.
pub(crate) fn split_ustar_path(path: &[u8]) -> Option<(&[u8], &[u8])> {
    if path.len() <= 100 {
        return Some((path, &[]));
    }
    let search_start = path.len().saturating_sub(100);
    let search_end = (path.len() - 1).min(PREFIX_LENGTH);
    for slash in search_start..=search_end {
        if path.get(slash) == Some(&b'/') {
            let prefix = &path[..slash];
            let name = &path[slash + 1..];
            if prefix.len() <= PREFIX_LENGTH && !name.is_empty() && name.len() <= 100 {
                return Some((name, prefix));
            }
        }
    }
    None
}

/// Sums a header with the checksum field counted as spaces.
pub(crate) fn header_checksum(block: &[u8]) -> u32 {
    let mut sum = 0u32;
    for (index, &byte) in block.iter().enumerate() {
        if (CHECKSUM_OFFSET..CHECKSUM_END).contains(&index) {
            sum += u32::from(b' ');
        }
        else {
            sum += u32::from(byte);
        }
    }
    sum
}

fn parse_checksum(field: &[u8]) -> Option<u32> {
    let value = parse_number(field)?;
    u32::try_from(value).ok()
}

fn c_string(bytes: &[u8]) -> Vec<u8> {
    match bytes.iter().position(|&byte| byte == 0) {
        Some(end) => bytes[..end].to_vec(),
        None => bytes.to_vec(),
    }
}

fn parse_number(field: &[u8]) -> Option<i64> {
    match field.first() {
        Some(0x80) | Some(0xff) => parse_base256(field),
        _ => parse_octal(field),
    }
}

fn parse_octal(field: &[u8]) -> Option<i64> {
    let mut cursor = 0;
    while cursor < field.len() && (field[cursor] == 0 || field[cursor] == b' ') {
        cursor += 1;
    }
    let start = cursor;
    while cursor < field.len() && (b'0'..=b'7').contains(&field[cursor]) {
        cursor += 1;
    }
    if start == cursor {
        return Some(0);
    }
    let mut value = 0i64;
    for &byte in &field[start..cursor] {
        value = value.checked_mul(8)?.checked_add(i64::from(byte - b'0'))?;
    }
    Some(value)
}

fn parse_base256(field: &[u8]) -> Option<i64> {
    let marker = field[0];
    let bits = 8 * (field.len() - 1);
    let mut magnitude = 0u128;
    for &byte in &field[1..] {
        magnitude = magnitude.checked_mul(256)?.checked_add(u128::from(byte))?;
    }
    let value = if marker == 0x80 {
        i128::try_from(magnitude).ok()?
    }
    else if marker == 0xff {
        -((1i128 << bits) - i128::try_from(magnitude).ok()?)
    }
    else {
        return None;
    };
    i64::try_from(value).ok()
}

fn write_number(field: &mut [u8], value: i64) {
    field.fill(0);
    let digits = field.len() - 1;
    let max_octal = (1u128 << (3 * digits)) - 1;
    let magnitude = (value as i128).unsigned_abs();
    if value >= 0 && magnitude <= max_octal {
        let mut digits_bytes = vec![b'0'; digits];
        let mut remaining = magnitude;
        for byte in digits_bytes.iter_mut().rev() {
            *byte = b'0' + (remaining & 7) as u8;
            remaining >>= 3;
        }
        field[..digits].copy_from_slice(&digits_bytes);
        return;
    }

    let bits = 8 * digits;
    let magnitude = if value >= 0 { magnitude } else { (1u128 << bits) - magnitude };
    field[0] = if value >= 0 { 0x80 } else { 0xff };
    for index in 0..digits {
        let shift = 8 * (digits - 1 - index);
        field[1 + index] = ((magnitude >> shift) & 0xff) as u8;
    }
}
