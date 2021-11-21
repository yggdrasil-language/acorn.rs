use core::fmt;

use super::error::{DerError, DerErrorKind, DerResult};

/// An OBJECT IDENTIFIER as its component arcs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Oid {
    arcs: Vec<u64>,
}

impl Oid {
    /// Build an OID from its component arcs. The first two arcs must be valid
    /// (`first < 3`, `second < 40` when `first < 2`).
    pub fn new(arcs: Vec<u64>) -> DerResult<Self> {
        if arcs.len() < 2 {
            return Err(DerError::new(DerErrorKind::MalformedContent, 0));
        }
        let (first, second) = (arcs[0], arcs[1]);
        if first > 2 || (first < 2 && second >= 40) {
            return Err(DerError::new(DerErrorKind::MalformedContent, 0));
        }
        Ok(Self { arcs })
    }

    /// Parse a dotted string such as `1.2.840.10045.2.1`.
    pub fn from_dotted(text: &str) -> DerResult<Self> {
        let mut arcs = Vec::new();
        for part in text.split('.') {
            let value: u64 = part.parse().map_err(|_| DerError::new(DerErrorKind::MalformedContent, 0))?;
            arcs.push(value);
        }
        Self::new(arcs)
    }

    /// The component arcs.
    pub fn arcs(&self) -> &[u64] {
        &self.arcs
    }

    /// Encode the content bytes (without tag and length).
    pub fn content(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let first = self.arcs[0];
        let second = self.arcs[1];
        let head = first * 40 + second;
        write_base128(head, &mut out);
        for arc in &self.arcs[2..] {
            write_base128(*arc, &mut out);
        }
        out
    }

    /// Decode content bytes into an OID.
    pub fn from_content(content: &[u8]) -> DerResult<Self> {
        if content.is_empty() {
            return Err(DerError::new(DerErrorKind::MalformedContent, 0));
        }
        let mut arcs = Vec::new();
        let mut position = 0usize;
        let mut first = true;
        while position < content.len() {
            let (value, next) = read_base128(content, position)?;
            position = next;
            if first {
                let head = if value > 79 {
                    2
                }
                else if value >= 40 {
                    1
                }
                else {
                    0
                };
                arcs.push(head);
                arcs.push(value - head * 40);
                first = false;
            }
            else {
                arcs.push(value);
            }
        }
        Self::new(arcs)
    }

    /// The dotted string form.
    pub fn dotted(&self) -> String {
        let mut text = String::new();
        for (index, arc) in self.arcs.iter().enumerate() {
            if index > 0 {
                text.push('.');
            }
            text.push_str(&arc.to_string());
        }
        text
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.dotted())
    }
}

/// Append one value in base-128 big-endian with continuation bits.
fn write_base128(value: u64, out: &mut Vec<u8>) {
    let mut buffer = [0u8; 10];
    let mut length = 0usize;
    let mut current = value;
    buffer[length] = (current & 0x7f) as u8;
    length += 1;
    current >>= 7;
    while current > 0 {
        buffer[length] = (current & 0x7f) as u8 | 0x80;
        length += 1;
        current >>= 7;
    }
    for index in (0..length).rev() {
        out.push(buffer[index]);
    }
}

/// Read one base-128 value, returning it and the offset after it.
fn read_base128(content: &[u8], mut position: usize) -> DerResult<(u64, usize)> {
    let mut value: u64 = 0;
    let mut iterations = 0usize;
    loop {
        let byte = *content.get(position).ok_or(DerError::new(DerErrorKind::Truncated, position))?;
        position += 1;
        iterations += 1;
        if iterations > 10 || (iterations == 10 && byte & 0x7f > 1) {
            return Err(DerError::new(DerErrorKind::MalformedContent, position));
        }
        value = (value << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok((value, position));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_round_trip() {
        for text in ["1.2.840.10045.2.1", "2.5.29.17", "1.3.132.0.34", "2.23.140.1.2.1", "0.0", "2.999.1"] {
            let oid = Oid::from_dotted(text).expect("parse");
            assert_eq!(oid.dotted(), text);
        }
    }

    #[test]
    fn content_round_trip() {
        for text in ["1.2.840.113549.1.1.11", "2.5.4.3", "1.2.840.10045.4.3.3", "1.3.6.1.5.5.7.3.1"] {
            let oid = Oid::from_dotted(text).expect("parse");
            let content = oid.content();
            let decoded = Oid::from_content(&content).expect("decode");
            assert_eq!(decoded, oid);
        }
    }

    #[test]
    fn known_encodings_match_der() {
        // 1.2.840.10045.2.1 -> 2A 86 48 CE 3D 02 01 (RFC 5480 id-ecPublicKey).
        let oid = Oid::from_dotted("1.2.840.10045.2.1").expect("parse");
        assert_eq!(oid.content(), vec![0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01]);
    }

    #[test]
    fn rejects_invalid_head_arcs() {
        assert!(Oid::new(vec![3, 0]).is_err());
        assert!(Oid::new(vec![1, 40]).is_err());
        assert!(Oid::new(vec![0]).is_err());
    }

    #[test]
    fn rejects_overlong_base128() {
        assert!(Oid::from_content(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]).is_err());
    }
}
