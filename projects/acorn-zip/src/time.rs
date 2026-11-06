//! ZIP modification-time facts.
//!
//! A ZIP member always carries a DOS date/time pair in its local and central
//! headers. Many writers also emit an extended-timestamp extra field (`0x5455`)
//! with second-resolution Unix time. Both are format facts here: this module
//! never converts to a host clock type and never applies a time zone.

use crate::eocd::{read_u16_le, read_u32_le};

/// Extra field id for the extended timestamp (`0x5455`).
pub const EXTENDED_TIMESTAMP_EXTRA_ID: u16 = 0x5455;

/// Oldest year a DOS date can represent.
pub const DOS_MIN_YEAR: u16 = 1980;
/// Newest year a DOS date can represent.
pub const DOS_MAX_YEAR: u16 = 2107;

/// Raw DOS date/time pair stored in a ZIP header.
///
/// The pair is kept as the two header words so an unrepresentable or zero value
/// survives indexing. Use [`ZipDosDateTime::parts`] to interpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZipDosDateTime {
    /// DOS time word: seconds, minutes and hours.
    pub time: u16,
    /// DOS date word: day, month and year offset from 1980.
    pub date: u16,
}

impl ZipDosDateTime {
    /// The all-zero pair writers use when they have no timestamp.
    pub const ZERO: Self = Self { time: 0, date: 0 };

    /// Creates a raw pair from the two header words.
    pub const fn new(time: u16, date: u16) -> Self {
        Self { time, date }
    }

    /// Decodes the pair into civil components, or `None` when the fields are zero
    /// or outside the ranges a DOS date can express.
    pub fn parts(self) -> Option<ZipDateTime> {
        let day = (self.date & 0x1f) as u8;
        let month = ((self.date >> 5) & 0x0f) as u8;
        let year = ((self.date >> 9) & 0x7f) as u16 + DOS_MIN_YEAR;
        let second = ((self.time & 0x1f) * 2) as u8;
        let minute = ((self.time >> 5) & 0x3f) as u8;
        let hour = ((self.time >> 11) & 0x1f) as u8;
        ZipDateTime::new(year, month, day, hour, minute, second)
    }
}

/// Civil date and time components of a DOS timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipDateTime {
    /// Year in `1980..=2107`.
    pub year: u16,
    /// Month in `1..=12`.
    pub month: u8,
    /// Day in `1..=31`.
    pub day: u8,
    /// Hour in `0..=23`.
    pub hour: u8,
    /// Minute in `0..=59`.
    pub minute: u8,
    /// Even second in `0..=58`, because DOS stores two-second steps.
    pub second: u8,
}

impl ZipDateTime {
    /// Validates components against the ranges a DOS date can encode.
    ///
    /// Odd seconds are rejected instead of rounded silently, because a round trip
    /// through this type must return the value the caller supplied.
    pub const fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        if year < DOS_MIN_YEAR || year > DOS_MAX_YEAR {
            return None;
        }
        if month == 0 || month > 12 || day == 0 || day > 31 {
            return None;
        }
        if hour > 23 || minute > 59 || second > 58 || second % 2 != 0 {
            return None;
        }
        Some(Self { year, month, day, hour, minute, second })
    }

    /// Encodes the components as a DOS date/time pair.
    pub const fn to_dos(self) -> Option<ZipDosDateTime> {
        if self.year < DOS_MIN_YEAR || self.year > DOS_MAX_YEAR {
            return None;
        }
        let date = ((self.year - DOS_MIN_YEAR) << 9) | ((self.month as u16) << 5) | self.day as u16;
        let time = ((self.hour as u16) << 11) | ((self.minute as u16) << 5) | (self.second as u16 >> 1);
        Some(ZipDosDateTime::new(time, date))
    }
}

/// Extended-timestamp values from extra field `0x5455`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZipExtendedTimestamp {
    /// Modification time in Unix seconds.
    pub modified_unix: Option<i64>,
    /// Access time in Unix seconds.
    pub accessed_unix: Option<i64>,
    /// Creation time in Unix seconds.
    pub created_unix: Option<i64>,
}

/// Reads an extended-timestamp extra block, if one is present.
///
/// The central directory only stores the modification time even when the flags
/// announce more fields, so fields that are not actually present stay `None`.
pub fn parse_extended_timestamp(extra: &[u8], extra_start: usize, extra_length: usize) -> Option<ZipExtendedTimestamp> {
    let extra_end = extra_start.checked_add(extra_length)?;
    if extra_end > extra.len() {
        return None;
    }
    let mut cursor = extra_start;
    while cursor + 4 <= extra_end {
        let header_id = read_u16_le(extra, cursor)?;
        let data_size = read_u16_le(extra, cursor + 2)? as usize;
        cursor += 4;
        let data_end = cursor.checked_add(data_size)?;
        if data_end > extra_end {
            return None;
        }
        if header_id != EXTENDED_TIMESTAMP_EXTRA_ID {
            cursor = data_end;
            continue;
        }
        let flags = *extra.get(cursor)?;
        let mut values = cursor + 1;
        let mut timestamp = ZipExtendedTimestamp::default();
        for (flag, slot) in [(0x01, 0), (0x02, 1), (0x04, 2)] {
            if flags & flag == 0 {
                continue;
            }
            if values + 4 > data_end {
                return None;
            }
            let value = read_u32_le(extra, values)? as i32;
            let value = i64::from(value);
            match slot {
                0 => timestamp.modified_unix = Some(value),
                1 => timestamp.accessed_unix = Some(value),
                _ => timestamp.created_unix = Some(value),
            }
            values += 4;
        }
        return Some(timestamp);
    }
    None
}
