//! `name` — name records.
//!
//! Platform, encoding, and language IDs follow the OpenType spec. We keep
//! them raw plus offer a couple of helpers to pick the most informative
//! Unicode/Western record for a given name ID.

use crate::reader::{ReadError, Reader};
use std::collections::HashMap;

/// Standard name IDs (per OpenType §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameId(pub u16);

impl NameId {
    pub const COPYRIGHT: NameId = NameId(0);
    pub const FAMILY: NameId = NameId(1);
    pub const SUBFAMILY: NameId = NameId(2);
    pub const UNIQUE_ID: NameId = NameId(3);
    pub const FULL: NameId = NameId(4);
    pub const VERSION: NameId = NameId(5);
    pub const POSTSCRIPT: NameId = NameId(6);
    pub const TRADEMARK: NameId = NameId(7);
    pub const MANUFACTURER: NameId = NameId(8);
    pub const DESIGNER: NameId = NameId(9);
    pub const DESCRIPTION: NameId = NameId(10);
    pub const VENDOR_URL: NameId = NameId(11);
    pub const DESIGNER_URL: NameId = NameId(12);
    pub const LICENSE: NameId = NameId(13);
    pub const LICENSE_URL: NameId = NameId(14);
    pub const TYPOGRAPHIC_FAMILY: NameId = NameId(16);
    pub const TYPOGRAPHIC_SUBFAMILY: NameId = NameId(17);
    pub const COMPATIBLE_FULL: NameId = NameId(18);
    pub const SAMPLE_TEXT: NameId = NameId(19);
    pub const PREFERRED_FAMILY: NameId = NameId(16);
    pub const PREFERRED_SUBFAMILY: NameId = NameId(17);

    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }
}

/// A single record in the `name` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRecord {
    /// Platform ID.
    pub platform: u16,
    /// Encoding ID (meaningful within platform).
    pub encoding: u16,
    /// Language ID (meaningful within platform).
    pub language: u16,
    /// Name ID (kind of metadata).
    pub name_id: u16,
    /// Record length in bytes.
    pub length: u16,
    /// Record offset from the start of the storage area (after header).
    pub offset: u16,
    /// Decoded string (if UTF-16BE or well-known encoding).
    pub decoded: Option<String>,
    /// Raw bytes (always present).
    pub raw: Vec<u8>,
}

/// Name table with all records.
#[derive(Debug, Clone)]
pub struct NameTable {
    /// Format 0 or 1.
    pub format: u16,
    /// All records in declaration order.
    pub records: Vec<NameRecord>,
    /// Storage area (bytes after the records).
    pub storage: Vec<u8>,
}

impl NameTable {
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        let start = r.position();
        let format = r.read_u16()?;
        let count = r.read_u16()?;
        let storage_offset = r.read_u16()?;

        // Post-format-1 also has langTagCount, but we accept it as extensions.
        let records_start = r.position();
        let storage_start = start + storage_offset as usize;

        let mut records = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let rec = NameRecord::parse(r)?;
            records.push(rec);
        }

        // Read storage area: everything from storage_start to end-of-table.
        let reader_limit = r.bytes(storage_start - start).map(|_| ());
        let _ = reader_limit;
        // Adjust reader bound so we can read the storage area.
        let total = r.limit - start;
        let storage_len = total.saturating_sub(storage_offset as usize);
        let storage_slice = if storage_start + storage_len <= r.limit && storage_start >= start {
            &r.bytes[storage_start..storage_start + storage_len]
        }
        else {
            // Fallback: clamp at end-of-file already handled by face.
            let clamped = storage_start.min(r.limit);
            let clamped_len = r.limit - clamped;
            &r.bytes[clamped..clamped + clamped_len]
        };
        let storage = storage_slice.to_vec();

        let mut table = Self { format, records, storage };
        table.decode_all();
        Ok(table)
    }

    fn decode_all(&mut self) {
        for rec in self.records.iter_mut() {
            let start = rec.offset as usize;
            let end = start + rec.length as usize;
            let raw = if end <= self.storage.len() { self.storage[start..end].to_vec() } else { Vec::new() };
            let decoded = decode_name_record(&raw, rec.platform, rec.encoding);
            rec.raw = raw;
            rec.decoded = decoded;
        }
    }

    /// Look up the best available string for a given name ID.
    ///
    /// Preference order: platform 3 (Windows), then 1 (Macintosh). Each
    /// platform prefers encoding 1 (Unicode / UnicodeBE).
    pub fn get(&self, name_id: NameId) -> Option<&str> {
        let mut best_3 = None;
        let mut best_1 = None;
        for rec in &self.records {
            if rec.name_id != name_id.0 {
                continue;
            }
            match rec.platform {
                3 if rec.encoding == 1 && best_3.is_none() => best_3 = rec.decoded.as_deref(),
                3 if best_3.is_none() => best_3 = rec.decoded.as_deref(),
                1 if best_1.is_none() => best_1 = rec.decoded.as_deref(),
                _ => {}
            }
        }
        best_3.or(best_1)
    }

    /// Collect all name IDs of interest into a map.
    pub fn as_map(&self) -> HashMap<NameId, String> {
        let mut out = HashMap::new();
        for nid in [
            NameId::COPYRIGHT,
            NameId::FAMILY,
            NameId::SUBFAMILY,
            NameId::FULL,
            NameId::POSTSCRIPT,
            NameId::VERSION,
            NameId::TRADEMARK,
            NameId::MANUFACTURER,
            NameId::DESIGNER,
            NameId::DESCRIPTION,
            NameId::LICENSE,
            NameId::LICENSE_URL,
            NameId::TYPOGRAPHIC_FAMILY,
            NameId::TYPOGRAPHIC_SUBFAMILY,
            NameId::PREFERRED_FAMILY,
            NameId::PREFERRED_SUBFAMILY,
            NameId::VENDOR_URL,
            NameId::DESIGNER_URL,
        ] {
            if let Some(s) = self.get(nid) {
                out.insert(nid, s.to_string());
            }
        }
        out
    }
}

impl NameRecord {
    fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        Ok(Self {
            platform: r.read_u16()?,
            encoding: r.read_u16()?,
            language: r.read_u16()?,
            name_id: r.read_u16()?,
            length: r.read_u16()?,
            offset: r.read_u16()?,
            decoded: None,
            raw: Vec::new(),
        })
    }
}

fn decode_name_record(bytes: &[u8], platform: u16, encoding: u16) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    match platform {
        1 => {
            // Macintosh: encoding 0 is MacRoman, 1 is MacJapanese (we keep raw ASCII).
            if encoding == 0 {
                // MacRoman is an 8-bit CP-10000 superset — fall back to Latin-1 best effort.
                let s = String::from_utf8_lossy(bytes).into_owned();
                Some(s)
            }
            else {
                let s = String::from_utf8_lossy(bytes).into_owned();
                Some(s)
            }
        }
        3 => {
            // Windows: encoding 1 is UTF-16BE.
            if encoding == 1 || encoding == 10 {
                let chars: Vec<u16> =
                    bytes.chunks(2).filter_map(|c| (c.len() == 2).then(|| u16::from_be_bytes([c[0], c[1]]))).collect();
                String::from_utf16(&chars).ok()
            }
            else {
                let s = String::from_utf8_lossy(bytes).into_owned();
                Some(s)
            }
        }
        _ => {
            let s = String::from_utf8_lossy(bytes).into_owned();
            Some(s)
        }
    }
}
