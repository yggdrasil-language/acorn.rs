use super::{
    error::{DerError, DerErrorKind, DerResult},
    oid::Oid,
    reader::Tlv,
    tag::Tag,
    time::DateTime,
};

/// A builder that emits strict DER bytes.
#[derive(Debug, Clone, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// An empty writer.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// The bytes written so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Consume the writer and return the encoded document.
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    /// Write one complete element from a tag and value bytes.
    pub fn write_tlv(&mut self, tag: Tag, value: &[u8]) {
        encode_header(tag, value.len(), &mut self.buf);
        self.buf.extend_from_slice(value);
    }

    /// Write a SEQUENCE whose children are produced by `build`.
    pub fn sequence(&mut self, build: impl FnOnce(&mut Writer)) {
        self.write_constructed(Tag::SEQUENCE, build);
    }

    /// Write a SET whose children are produced by `build`.
    pub fn set(&mut self, build: impl FnOnce(&mut Writer)) {
        self.write_constructed(Tag::SET, build);
    }

    /// Write a context-specific constructed element with the given number.
    pub fn context(&mut self, number: u32, build: impl FnOnce(&mut Writer)) {
        self.write_constructed(Tag::context(number, true), build);
    }

    /// Write a context-specific primitive element from raw bytes.
    pub fn context_primitive(&mut self, number: u32, value: &[u8]) {
        self.write_tlv(Tag::context(number, false), value);
    }

    /// Write a constructed element whose children are produced by `build`.
    pub fn write_constructed(&mut self, tag: Tag, build: impl FnOnce(&mut Writer)) {
        let mut inner = Writer::new();
        build(&mut inner);
        self.write_tlv(tag, &inner.buf);
    }

    /// Write a non-negative INTEGER from a `u64`.
    pub fn write_u64(&mut self, value: u64) {
        let bytes = value.to_be_bytes();
        let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(bytes.len() - 1);
        self.write_integer(&bytes[start..]);
    }

    /// Write a non-negative INTEGER from big-endian magnitude bytes.
    pub fn write_integer(&mut self, magnitude: &[u8]) {
        let mut bytes = magnitude;
        while bytes.len() > 1 && bytes[0] == 0 {
            bytes = &bytes[1..];
        }
        let mut value = Vec::with_capacity(bytes.len() + 1);
        if bytes.first().is_none_or(|byte| byte & 0x80 != 0) {
            value.push(0);
        }
        value.extend_from_slice(bytes);
        self.write_tlv(Tag::INTEGER, &value);
    }

    /// Write an OID element.
    pub fn write_oid(&mut self, oid: &Oid) {
        self.write_tlv(Tag::OID, &oid.content());
    }

    /// Write an OID from its dotted string.
    pub fn write_oid_dotted(&mut self, text: &str) -> DerResult<()> {
        let oid = Oid::from_dotted(text).map_err(|error| DerError { offset: self.buf.len(), ..error })?;
        self.write_oid(&oid);
        Ok(())
    }

    /// Write a NULL element.
    pub fn write_null(&mut self) {
        self.write_tlv(Tag::NULL, &[]);
    }

    /// Write a BOOLEAN element with the DER canonical TRUE (`0xFF`).
    pub fn write_boolean(&mut self, value: bool) {
        self.write_tlv(Tag::BOOLEAN, &[if value { 0xFF } else { 0x00 }]);
    }

    /// Write a BIT STRING with the given content and unused bits.
    pub fn write_bitstring(&mut self, bytes: &[u8], unused_bits: u8) {
        let mut value = Vec::with_capacity(bytes.len() + 1);
        value.push(unused_bits);
        value.extend_from_slice(bytes);
        self.write_tlv(Tag::BIT_STRING, &value);
    }

    /// Write an OCTET STRING.
    pub fn write_octetstring(&mut self, bytes: &[u8]) {
        self.write_tlv(Tag::OCTET_STRING, bytes);
    }

    /// Write a UTF8String.
    pub fn write_utf8(&mut self, text: &str) {
        self.write_tlv(Tag::UTF8_STRING, text.as_bytes());
    }

    /// Write a PrintableString, rejecting characters outside its set.
    pub fn write_printable(&mut self, text: &str) -> DerResult<()> {
        if !text.bytes().all(|byte| {
            matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b' ' | b'\'' | b'(' | b')' | b'+' | b',' | b'-' | b'.' | b'/' | b':' | b'=' | b'?')
        }) {
            return Err(DerError::new(DerErrorKind::MalformedContent, self.buf.len()));
        }
        self.write_tlv(Tag::PRINTABLE_STRING, text.as_bytes());
        Ok(())
    }

    /// Write an IA5String, rejecting bytes outside printable ASCII.
    pub fn write_ia5(&mut self, text: &str) -> DerResult<()> {
        if !text.bytes().all(|byte| byte.is_ascii_graphic() || byte == b' ') {
            return Err(DerError::new(DerErrorKind::MalformedContent, self.buf.len()));
        }
        self.write_tlv(Tag::IA5_STRING, text.as_bytes());
        Ok(())
    }

    /// Write a UTCTime (two-digit year) element.
    pub fn write_utctime(&mut self, value: &DateTime) -> DerResult<()> {
        if !(1950..2050).contains(&value.year) {
            return Err(DerError::new(DerErrorKind::MalformedContent, self.buf.len()));
        }
        self.write_tlv(Tag::UTCTIME, value.utc_text().as_bytes());
        Ok(())
    }

    /// Write a GeneralizedTime (four-digit year) element.
    pub fn write_generalized_time(&mut self, value: &DateTime) -> DerResult<()> {
        self.write_tlv(Tag::GENERALIZED_TIME, value.generalized_text().as_bytes());
        Ok(())
    }

    /// Copy an already-parsed element verbatim (tag plus value).
    pub fn write_element(&mut self, element: &Tlv<'_>) {
        self.write_tlv(element.tag, element.value);
    }

    /// Append a complete element that was already encoded in strict DER.
    pub fn write_raw(&mut self, element: &[u8]) {
        self.buf.extend_from_slice(element);
    }
}

/// Encode one complete DER element from a tag and value bytes.
pub fn encode_element(tag: Tag, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 4);
    encode_header(tag, value.len(), &mut out);
    out.extend_from_slice(value);
    out
}

/// Encode `tag` and `length` into the output buffer.
pub fn encode_header(tag: Tag, length: usize, out: &mut Vec<u8>) {
    out.push(tag.lead_byte());
    if tag.needs_extended_number() {
        // Re-emit the base-128 number after the lead byte.
        let mut buffer = [0u8; 5];
        let mut size = 0usize;
        let mut current = tag.number;
        buffer[size] = (current & 0x7f) as u8;
        size += 1;
        current >>= 7;
        while current > 0 {
            buffer[size] = (current & 0x7f) as u8 | 0x80;
            size += 1;
            current >>= 7;
        }
        for index in (0..size).rev() {
            out.push(buffer[index]);
        }
    }
    if length < 0x80 {
        out.push(length as u8);
    }
    else {
        let bytes = length.to_be_bytes();
        let start = bytes.iter().position(|byte| *byte != 0).expect("non-zero length byte");
        let count = bytes.len() - start;
        out.push(0x80 | count as u8);
        out.extend_from_slice(&bytes[start..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::Reader;

    #[test]
    fn sequence_of_integer_and_oid_round_trips() {
        let mut writer = Writer::new();
        writer.sequence(|inner| {
            inner.write_u64(128);
            inner.write_oid_dotted("1.2.840.10045.2.1").expect("oid");
            inner.write_null();
        });
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let mut children = reader.expect_sequence().expect("sequence");
        assert_eq!(children.expect_u64().expect("int"), 128);
        assert_eq!(children.expect_oid().expect("oid").dotted(), "1.2.840.10045.2.1");
        children.expect_null().expect("null");
        assert!(children.is_empty());
        reader.expect_end().expect("end");
    }

    #[test]
    fn integer_padding_matches_der_rules() {
        let mut writer = Writer::new();
        writer.write_integer(&[0x80]);
        assert_eq!(writer.as_bytes(), &[0x02, 0x02, 0x00, 0x80]);
        let mut writer = Writer::new();
        writer.write_integer(&[0x00, 0x7F]);
        assert_eq!(writer.as_bytes(), &[0x02, 0x01, 0x7F]);
        let mut writer = Writer::new();
        writer.write_integer(&[0x00]);
        assert_eq!(writer.as_bytes(), &[0x02, 0x01, 0x00]);
    }

    #[test]
    fn long_lengths_encode_minimally() {
        for length in [127usize, 128, 255, 256, 70_000] {
            let mut writer = Writer::new();
            writer.write_octetstring(&vec![0u8; length]);
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(reader.expect_octetstring().expect("round trip").len(), length);
        }
    }

    #[test]
    fn context_constructed_round_trips() {
        let mut writer = Writer::new();
        writer.context(0, |inner| inner.write_u64(3));
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let element = reader.expect_context(0, true).expect("context");
        assert_eq!(element.reader().expect_u64().expect("child"), 3);
    }

    #[test]
    fn printable_and_ia5_reject_bad_bytes() {
        let mut writer = Writer::new();
        assert!(writer.write_printable("CN=ok!").is_err());
        assert!(writer.write_printable("valeur").is_ok());
        let mut writer = Writer::new();
        assert!(writer.write_ia5("a*b.example").is_ok());
        assert!(writer.write_ia5("bad\u{1}").is_err());
    }

    #[test]
    fn bitstring_writes_unused_prefix() {
        let mut writer = Writer::new();
        writer.write_bitstring(&[0b0100_0000], 1);
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let value = reader.expect_bitstring().expect("bitstring");
        assert_eq!(value.unused_bits, 1);
        assert_eq!(value.bytes, &[0b0100_0000]);
    }
}
