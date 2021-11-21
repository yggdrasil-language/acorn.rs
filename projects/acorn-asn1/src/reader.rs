use super::{
    error::{DerError, DerErrorKind, DerResult},
    oid::Oid,
    tag::{Class, Tag},
};

/// One decoded type-length-value element borrowed from the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tlv<'a> {
    /// The element tag.
    pub tag: Tag,
    /// The value bytes, excluding tag and length.
    pub value: &'a [u8],
    /// Byte offset of the tag within the original input.
    pub offset: usize,
}

impl<'a> Tlv<'a> {
    /// A reader over this element's value.
    pub const fn reader(&self) -> Reader<'a> {
        Reader::new(self.value)
    }

    /// Decode this element's value as UTF-8 text.
    pub fn text(&self) -> DerResult<&'a str> {
        core::str::from_utf8(self.value).map_err(|_| DerError::new(DerErrorKind::MalformedContent, self.offset))
    }
}

/// A BIT STRING payload split into its unused-bit count and content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitString<'a> {
    /// Bits ignored in the final content byte, always in `0..=7`.
    pub unused_bits: u8,
    /// The content bytes after the unused-bit prefix.
    pub bytes: &'a [u8],
}

/// A cursor that decodes DER elements from a byte slice.
#[derive(Debug, Clone, Copy)]
pub struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Start a reader over the whole slice.
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// Bytes not yet consumed.
    pub const fn remaining(&self) -> usize {
        self.data.len() - self.offset
    }

    /// Whether every byte has been consumed.
    pub const fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// The absolute byte offset of the next element.
    pub const fn position(&self) -> usize {
        self.offset
    }

    /// Peek at the next element's tag without consuming it.
    pub fn peek_tag(&self) -> DerResult<Tag> {
        let (tag, _) = decode_tag(self.data, self.offset)?;
        Ok(tag)
    }

    /// Consume the next element whole.
    pub fn read_tlv(&mut self) -> DerResult<Tlv<'a>> {
        let offset = self.offset;
        let (tag, cursor) = decode_tag(self.data, offset)?;
        let (length, cursor) = decode_length(self.data, cursor)?;
        let end = cursor.checked_add(length).ok_or(DerError::new(DerErrorKind::BadLength, cursor))?;
        let value = self.data.get(cursor..end).ok_or(DerError::new(DerErrorKind::Truncated, end))?;
        self.offset = end;
        Ok(Tlv { tag, value, offset })
    }

    /// Consume one element and require its tag to match exactly.
    pub fn expect(&mut self, tag: Tag) -> DerResult<Tlv<'a>> {
        let element = self.read_tlv()?;
        if element.tag != tag {
            return Err(DerError::tag_mismatch(tag.number, element.tag.number, element.offset));
        }
        Ok(element)
    }

    /// Consume one element requiring the class and number, ignoring the constructed bit.
    pub fn expect_number(&mut self, tag: Tag) -> DerResult<Tlv<'a>> {
        let element = self.read_tlv()?;
        if !element.tag.same_number(tag) {
            return Err(DerError::tag_mismatch(tag.number, element.tag.number, element.offset));
        }
        Ok(element)
    }

    /// Consume a context-specific element with the given number.
    pub fn expect_context(&mut self, number: u32, constructed: bool) -> DerResult<Tlv<'a>> {
        self.expect(Tag::context(number, constructed))
    }

    /// Consume a SEQUENCE and return a reader over its children.
    pub fn expect_sequence(&mut self) -> DerResult<Reader<'a>> {
        Ok(self.expect(Tag::SEQUENCE)?.reader())
    }

    /// Consume a SET and return a reader over its children.
    pub fn expect_set(&mut self) -> DerResult<Reader<'a>> {
        Ok(self.expect(Tag::SET)?.reader())
    }

    /// Consume a NULL.
    pub fn expect_null(&mut self) -> DerResult<()> {
        let element = self.expect(Tag::NULL)?;
        if !element.value.is_empty() {
            return Err(DerError::new(DerErrorKind::MalformedContent, element.offset));
        }
        Ok(())
    }

    /// Consume a BOOLEAN. DER encodes TRUE as `0xFF`; `0x01` is also accepted.
    pub fn expect_boolean(&mut self) -> DerResult<bool> {
        let element = self.expect(Tag::BOOLEAN)?;
        match element.value {
            [0] => Ok(false),
            [1] | [0xFF] => Ok(true),
            _ => Err(DerError::new(DerErrorKind::MalformedContent, element.offset)),
        }
    }

    /// Consume a non-negative INTEGER that fits in `u64`.
    pub fn expect_u64(&mut self) -> DerResult<u64> {
        let element = self.expect(Tag::INTEGER)?;
        let bytes = minimal_integer(element.value, element.offset)?;
        if bytes.is_empty() || bytes[0] & 0x80 != 0 {
            return Err(DerError::new(DerErrorKind::IntegerOverflow, element.offset));
        }
        if bytes.len() > 8 {
            return Err(DerError::new(DerErrorKind::IntegerOverflow, element.offset));
        }
        let mut value = 0u64;
        for byte in bytes {
            value = (value << 8) | u64::from(*byte);
        }
        Ok(value)
    }

    /// Consume a non-negative INTEGER and return its big-endian magnitude bytes.
    pub fn expect_integer_bytes(&mut self) -> DerResult<&'a [u8]> {
        let element = self.expect(Tag::INTEGER)?;
        let bytes = minimal_integer(element.value, element.offset)?;
        if bytes.is_empty() || bytes[0] & 0x80 != 0 {
            return Err(DerError::new(DerErrorKind::MalformedContent, element.offset));
        }
        // Strip the positive-sign padding byte when present.
        let magnitude = if bytes.len() > 1 && bytes[0] == 0 { &bytes[1..] } else { bytes };
        Ok(magnitude)
    }

    /// Consume an OBJECT IDENTIFIER.
    pub fn expect_oid(&mut self) -> DerResult<Oid> {
        let element = self.expect(Tag::OID)?;
        Oid::from_content(element.value).map_err(|error| DerError { offset: element.offset, ..error })
    }

    /// Consume a BIT STRING and validate its padding.
    pub fn expect_bitstring(&mut self) -> DerResult<BitString<'a>> {
        let element = self.expect(Tag::BIT_STRING)?;
        let (&unused, bytes) =
            element.value.split_first().ok_or(DerError::new(DerErrorKind::MalformedContent, element.offset))?;
        if unused > 7 || (unused > 0 && bytes.is_empty()) {
            return Err(DerError::new(DerErrorKind::MalformedContent, element.offset));
        }
        if unused > 0 && bytes.last().is_some_and(|last| last & (0xFFu8 >> (8 - unused)) != 0) {
            return Err(DerError::new(DerErrorKind::MalformedContent, element.offset));
        }
        Ok(BitString { unused_bits: unused, bytes })
    }

    /// Consume an OCTET STRING.
    pub fn expect_octetstring(&mut self) -> DerResult<&'a [u8]> {
        Ok(self.expect(Tag::OCTET_STRING)?.value)
    }

    /// Consume a UTF8String.
    pub fn expect_utf8(&mut self) -> DerResult<&'a str> {
        let element = self.expect(Tag::UTF8_STRING)?;
        self.text_of(element)
    }

    /// Consume a PrintableString.
    pub fn expect_printable(&mut self) -> DerResult<&'a str> {
        let element = self.expect(Tag::PRINTABLE_STRING)?;
        if !element.value.iter().all(is_printable) {
            return Err(DerError::new(DerErrorKind::MalformedContent, element.offset));
        }
        self.text_of(element)
    }

    /// Consume an IA5String.
    pub fn expect_ia5(&mut self) -> DerResult<&'a str> {
        let element = self.expect(Tag::IA5_STRING)?;
        if !element.value.iter().all(|byte| byte.is_ascii_graphic() || *byte == b' ') {
            return Err(DerError::new(DerErrorKind::MalformedContent, element.offset));
        }
        self.text_of(element)
    }

    /// Decode the content of any string-ish element whose tag the caller already matched.
    pub fn text_of(&self, element: Tlv<'a>) -> DerResult<&'a str> {
        element.text()
    }

    /// Require that the input is fully consumed.
    pub fn expect_end(&self) -> DerResult<()> {
        if self.is_empty() { Ok(()) } else { Err(DerError::new(DerErrorKind::TrailingData, self.offset)) }
    }

    /// Collect every remaining element, requiring the input to end afterwards.
    pub fn read_all(&mut self) -> DerResult<Vec<Tlv<'a>>> {
        let mut items = Vec::new();
        while !self.is_empty() {
            items.push(self.read_tlv()?);
        }
        Ok(items)
    }
}

/// Decode a tag starting at `position`, returning it and the offset after it.
fn decode_tag(data: &[u8], position: usize) -> DerResult<(Tag, usize)> {
    let lead = *data.get(position).ok_or(DerError::new(DerErrorKind::Truncated, position))?;
    let class = Class::from_bits(lead);
    let constructed = lead & 0b0010_0000 != 0;
    let low = lead & 0b0001_1111;
    if low < 31 {
        return Ok((Tag { class, constructed, number: u32::from(low) }, position + 1));
    }
    let mut cursor = position + 1;
    let mut number: u32 = 0;
    let mut iterations = 0usize;
    loop {
        let byte = *data.get(cursor).ok_or(DerError::new(DerErrorKind::Truncated, cursor))?;
        cursor += 1;
        iterations += 1;
        if iterations > 5 || (iterations == 1 && byte == 0x80) {
            return Err(DerError::new(DerErrorKind::MalformedContent, cursor));
        }
        number = number
            .checked_mul(128)
            .and_then(|value| value.checked_add(u32::from(byte & 0x7f)))
            .ok_or(DerError::new(DerErrorKind::MalformedContent, cursor))?;
        if byte & 0x80 == 0 {
            return Ok((Tag { class, constructed, number }, cursor));
        }
    }
}

/// Decode a definite length starting at `position`, returning it and the offset after it.
fn decode_length(data: &[u8], position: usize) -> DerResult<(usize, usize)> {
    let lead = *data.get(position).ok_or(DerError::new(DerErrorKind::Truncated, position))?;
    if lead < 0x80 {
        return Ok((usize::from(lead), position + 1));
    }
    if lead == 0x80 {
        return Err(DerError::new(DerErrorKind::BadLength, position));
    }
    let count = usize::from(lead & 0x7f);
    if count > 8 {
        return Err(DerError::new(DerErrorKind::BadLength, position));
    }
    let end = position.checked_add(1 + count).ok_or(DerError::new(DerErrorKind::BadLength, position))?;
    let bytes = data.get(position + 1..end).ok_or(DerError::new(DerErrorKind::Truncated, end))?;
    if count > 1 && bytes[0] == 0 {
        return Err(DerError::new(DerErrorKind::NonMinimalLength, position));
    }
    let mut length: usize = 0;
    for byte in bytes {
        length = length
            .checked_mul(256)
            .and_then(|value| value.checked_add(usize::from(*byte)))
            .ok_or(DerError::new(DerErrorKind::BadLength, position))?;
    }
    if count == 1 && length < 0x80 {
        return Err(DerError::new(DerErrorKind::NonMinimalLength, position));
    }
    Ok((length, end))
}

/// Validate the minimal two's-complement form of an INTEGER and return its value bytes.
fn minimal_integer(bytes: &[u8], offset: usize) -> DerResult<&[u8]> {
    if bytes.is_empty() {
        return Err(DerError::new(DerErrorKind::MalformedContent, offset));
    }
    if bytes.len() > 1 {
        let redundant_positive = bytes[0] == 0x00 && bytes[1] & 0x80 == 0;
        let redundant_negative = bytes[0] == 0xFF && bytes[1] & 0x80 != 0;
        if redundant_positive || redundant_negative {
            return Err(DerError::new(DerErrorKind::NonMinimalInteger, offset));
        }
    }
    Ok(bytes)
}

/// True for the X.680 PrintableString character set.
fn is_printable(byte: &u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b' ' | b'\'' | b'(' | b')' | b'+' | b',' | b'-' | b'.' | b'/' | b':' | b'=' | b'?')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_long_lengths() {
        // SEQUENCE { NULL }
        let element = {
            let mut reader = Reader::new(&[0x30, 0x02, 0x05, 0x00]);
            reader.read_tlv().expect("short form")
        };
        assert_eq!(element.tag, Tag::SEQUENCE);
        assert_eq!(element.value, &[0x05, 0x00]);
        // A 130-byte OCTET STRING uses the 0x81 long form.
        let mut bytes = vec![0x04, 0x81, 0x82];
        bytes.extend(core::iter::repeat_n(0xAB, 130));
        let mut reader = Reader::new(&bytes);
        let value = reader.expect_octetstring().expect("long form");
        assert_eq!(value.len(), 130);
    }

    #[test]
    fn rejects_non_minimal_lengths() {
        // 0x81 0x7F encodes 127, which must use the short form.
        let mut bytes = vec![0x04, 0x81, 0x7F];
        bytes.extend(core::iter::repeat_n(0u8, 127));
        assert_eq!(Reader::new(&bytes).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::NonMinimalLength));
        // 0x82 0x00 0xFF is a non-minimal 255.
        let bytes = [0x04u8, 0x82, 0x00, 0xFF];
        assert_eq!(Reader::new(&bytes).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::NonMinimalLength));
    }

    #[test]
    fn rejects_indefinite_length() {
        let bytes = [0x30u8, 0x80];
        assert_eq!(Reader::new(&bytes).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::BadLength));
    }

    #[test]
    fn integers_validate_minimality() {
        // 0x00 0x05 is a non-minimal positive 5.
        let bytes = [0x02u8, 0x02, 0x00, 0x05];
        assert_eq!(Reader::new(&bytes).expect_u64().err().map(|error| error.kind), Some(DerErrorKind::NonMinimalInteger));
        // 0x00 0x80 is the minimal positive 128.
        let bytes = [0x02u8, 0x02, 0x00, 0x80];
        assert_eq!(Reader::new(&bytes).expect_u64().expect("128"), 128);
        // Negative integers are rejected by expect_u64.
        let bytes = [0x02u8, 0x01, 0xFF];
        assert_eq!(Reader::new(&bytes).expect_u64().err().map(|error| error.kind), Some(DerErrorKind::IntegerOverflow));
    }

    #[test]
    fn tag_mismatch_reports_both_tags() {
        let bytes = [0x02u8, 0x01, 0x00];
        let error = Reader::new(&bytes).expect(Tag::NULL).expect_err("must fail");
        assert_eq!(error.kind, DerErrorKind::TagMismatch);
        assert_eq!(error.expected, Some(0x05));
        assert_eq!(error.actual, Some(0x02));
    }

    #[test]
    fn bitstring_padding_is_enforced() {
        // 3 unused bits with a dirty pad byte.
        let bytes = [0x03u8, 0x02, 0x03, 0b0000_0101];
        assert_eq!(Reader::new(&bytes).expect_bitstring().err().map(|error| error.kind), Some(DerErrorKind::MalformedContent));
        let bytes = [0x03u8, 0x02, 0x03, 0b0000_0000];
        let value = Reader::new(&bytes).expect_bitstring().expect("clean pad");
        assert_eq!(value.unused_bits, 3);
        assert_eq!(value.bytes, &[0]);
    }

    #[test]
    fn context_tags_round_trip_through_expect() {
        let bytes = [0xA0u8, 0x00];
        let element = Reader::new(&bytes).expect_context(0, true).expect("context 0");
        assert!(element.value.is_empty());
    }

    #[test]
    fn high_tag_number_form() {
        // CONTEXT 31 constructed -> 0xBF 0x1F
        let bytes = [0xBFu8, 0x1F, 0x00];
        let element = Reader::new(&bytes).expect_context(31, true).expect("extended tag");
        assert!(element.value.is_empty());
        // Non-minimal extended number 0x80 0x00 must fail.
        let bytes = [0xBFu8, 0x80, 0x00, 0x00];
        assert!(Reader::new(&bytes).read_tlv().is_err());
    }
}
