//! DER round trips and boundary negatives for `acorn-asn1`.

use acorn_asn1::{DateTime, DerErrorKind, Reader, Tag, Writer, encode_element, oids};

/// The published ISRG Root X1 certificate, first 47 bytes of its DER.
///
/// The bytes are the pinned anchor from ACME's LPKI issuer; the prefix stops
/// after the `AlgorithmIdentifier` so the file stays a minimal known-answer
/// vector instead of a full certificate.
const ISRG_ROOT_X1_PREFIX: &[u8] = &[
    0x30, 0x82, 0x05, 0x6b, 0x30, 0x82, 0x03, 0x53, 0xa0, 0x03, 0x02, 0x01, 0x02, 0x02, 0x11, 0x00, 0x82, 0x10, 0xcf, 0xb0,
    0xd2, 0x40, 0xe3, 0x59, 0x44, 0x63, 0xe0, 0xbb, 0x63, 0x82, 0x8b, 0x00, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86,
    0xf7, 0x0d, 0x01, 0x01, 0x0b, 0x05, 0x00,
];

#[test]
fn isrg_root_x1_prefix_parses_as_der() {
    // The outer SEQUENCE declares 1387 value bytes, so a truncated prefix
    // must fail to read whole.
    let mut outer = Reader::new(ISRG_ROOT_X1_PREFIX);
    assert_eq!(outer.read_tlv().err().map(|error| error.kind), Some(DerErrorKind::Truncated));
    // Skip the outer certificate header and the TBSCertificate header.
    let mut body = Reader::new(&ISRG_ROOT_X1_PREFIX[8..]);
    let version = body.expect_context(0, true).expect("version [0]");
    assert_eq!(version.reader().expect_u64().expect("version integer"), 2);
    assert_eq!(
        body.expect_integer_bytes().expect("serial"),
        &[0x82, 0x10, 0xcf, 0xb0, 0xd2, 0x40, 0xe3, 0x59, 0x44, 0x63, 0xe0, 0xbb, 0x63, 0x82, 0x8b, 0x00]
    );
    let mut algorithm = body.expect_sequence().expect("algorithm identifier");
    assert_eq!(algorithm.expect_oid().expect("oid"), oids::sha256_with_rsa());
    algorithm.expect_null().expect("absent parameters");
    assert!(algorithm.is_empty());
    assert!(body.is_empty());
}

#[test]
fn full_document_round_trip() {
    let when = DateTime { year: 2023, month: 11, day: 30, hour: 12, minute: 45, second: 6 };
    let later = DateTime { year: 2049, month: 12, day: 31, hour: 23, minute: 59, second: 59 };
    let mut writer = Writer::new();
    writer.sequence(|body| {
        body.context(0, |inner| inner.write_u64(2));
        body.write_integer(&[0x82, 0x10]);
        body.write_oid(&oids::sha256_with_rsa());
        body.write_boolean(true);
        body.write_utf8("ISRG Root X1");
        body.write_printable("valeur").expect("printable");
        body.write_ia5("a@example.com").expect("ia5");
        body.write_bitstring(&[0x03, 0x01, 0x07], 0);
        body.write_octetstring(&[0xAA; 200]);
        body.write_utctime(&when).expect("utctime");
        body.write_generalized_time(&later).expect("generalized time");
        body.write_null();
    });
    let bytes = writer.finish();
    let mut reader = Reader::new(&bytes);
    let mut body = reader.expect_sequence().expect("sequence");
    assert_eq!(body.expect_context(0, true).expect("version").reader().expect_u64().expect("integer"), 2);
    assert_eq!(body.expect_integer_bytes().expect("serial"), &[0x82, 0x10]);
    assert_eq!(body.expect_oid().expect("oid"), oids::sha256_with_rsa());
    assert!(body.expect_boolean().expect("boolean"));
    assert_eq!(body.expect_utf8().expect("utf8"), "ISRG Root X1");
    assert_eq!(body.expect_printable().expect("printable"), "valeur");
    assert_eq!(body.expect_ia5().expect("ia5"), "a@example.com");
    let bits = body.expect_bitstring().expect("bitstring");
    assert_eq!(bits.bytes, &[0x03, 0x01, 0x07]);
    assert_eq!(body.expect_octetstring().expect("octetstring").len(), 200);
    let utc = body.expect(Tag::UTCTIME).expect("utctime element");
    assert_eq!(DateTime::parse(utc.value).expect("utc parse"), when);
    let general = body.expect(Tag::GENERALIZED_TIME).expect("generalized element");
    assert_eq!(DateTime::parse(general.value).expect("generalized parse"), later);
    body.expect_null().expect("null");
    assert!(body.is_empty());
    reader.expect_end().expect("end");
}

#[test]
fn canonical_element_bytes() {
    assert_eq!(encode_element(Tag::NULL, &[]), [0x05, 0x00]);
    assert_eq!(encode_element(Tag::INTEGER, &[0x00, 0x80]), [0x02, 0x02, 0x00, 0x80]);
    // 2.5.4.3 commonName is `55 04 03`, the X.500 anchor visible in the root
    // prefix above (`06 03 55 04 03`).
    assert_eq!(encode_element(Tag::OID, &oids::common_name().content()), [0x06, 0x03, 0x55, 0x04, 0x03]);
    // DER canonical TRUE is 0xFF.
    let mut writer = Writer::new();
    writer.write_boolean(true);
    assert_eq!(writer.as_bytes(), &[0x01, 0x01, 0xFF]);
    // The reader accepts 0xFF and 0x01 for TRUE and nothing else.
    assert!(Reader::new(&[0x01, 0x01, 0x01]).expect_boolean().expect("0x01 true"));
    assert_eq!(
        Reader::new(&[0x01, 0x01, 0x02]).expect_boolean().err().map(|error| error.kind),
        Some(DerErrorKind::MalformedContent)
    );
}

#[test]
fn truncated_headers_and_values() {
    // Long-form length whose bytes are cut off.
    assert_eq!(Reader::new(&[0x30, 0x82, 0x05]).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::Truncated));
    // Value shorter than the declared length.
    assert_eq!(Reader::new(&[0x04, 0x03, 0x01, 0x02]).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::Truncated));
    // Missing length byte entirely.
    assert_eq!(Reader::new(&[0x30]).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::Truncated));
}

#[test]
fn definite_and_minimal_lengths_only() {
    // BER indefinite length is rejected wherever a length is read.
    assert_eq!(Reader::new(&[0x30, 0x80, 0x05, 0x00]).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::BadLength));
    // 0x81 0x7F encodes 127, which must use the short form.
    let mut bytes = vec![0x04, 0x81, 0x7F];
    bytes.extend(core::iter::repeat_n(0u8, 127));
    assert_eq!(Reader::new(&bytes).read_tlv().err().map(|error| error.kind), Some(DerErrorKind::NonMinimalLength));
    // A long form with a leading zero length byte is non-minimal.
    assert_eq!(
        Reader::new(&[0x04, 0x82, 0x00, 0xFF]).read_tlv().err().map(|error| error.kind),
        Some(DerErrorKind::NonMinimalLength)
    );
    // More than eight length bytes is out of range.
    assert_eq!(
        Reader::new(&[0x04, 0x89, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            .read_tlv()
            .err()
            .map(|error| error.kind),
        Some(DerErrorKind::BadLength)
    );
}

#[test]
fn oid_rejects_non_canonical_base128() {
    // Eleven continuation bytes cannot fit a u64 arc and are rejected.
    let mut content = vec![0x80u8; 10];
    content.push(0x01);
    let element = encode_element(Tag::OID, &content);
    assert_eq!(Reader::new(&element).expect_oid().err().map(|error| error.kind), Some(DerErrorKind::MalformedContent));
    // An empty OID content is malformed.
    let element = encode_element(Tag::OID, &[]);
    assert_eq!(Reader::new(&element).expect_oid().err().map(|error| error.kind), Some(DerErrorKind::MalformedContent));
}

#[test]
fn time_formats_are_enforced() {
    // UTCTime keeps the RFC 5280 two-digit-year window.
    let element = encode_element(Tag::UTCTIME, b"231130120000Z");
    let mut reader = Reader::new(&element);
    let utc = reader.expect(Tag::UTCTIME).expect("utctime");
    assert_eq!(DateTime::parse(utc.value).expect("parse").year, 2023);
    // GeneralizedTime requires the four-digit year form.
    let element = encode_element(Tag::GENERALIZED_TIME, b"20491231235959Z");
    let mut reader = Reader::new(&element);
    let general = reader.expect(Tag::GENERALIZED_TIME).expect("generalized");
    assert_eq!(DateTime::parse(general.value).expect("parse").year, 2049);
    // Missing Zulu suffix.
    assert!(DateTime::parse(b"231130120000").is_err());
    // Fractional seconds are rejected: stripping the dot leaves 13 digits.
    assert!(DateTime::parse(b"231130120000.5Z").is_err());
    // A local offset form is rejected.
    assert!(DateTime::parse(b"231130120000+0800").is_err());
    // The writer refuses UTCTime outside 1950..2050.
    let mut writer = Writer::new();
    assert!(writer.write_utctime(&DateTime { year: 2050, month: 1, day: 1, hour: 0, minute: 0, second: 0 }).is_err());
}

#[test]
fn trailing_data_and_tag_mismatch_report_offsets() {
    let mut reader = Reader::new(&[0x05u8, 0x00, 0x05]);
    reader.expect(Tag::NULL).expect("first null");
    assert_eq!(reader.expect_end().err().map(|error| error.kind), Some(DerErrorKind::TrailingData));
    let error = Reader::new(&[0x02, 0x01, 0x00]).expect(Tag::OCTET_STRING).expect_err("wrong tag");
    assert_eq!((error.kind, error.expected, error.actual), (DerErrorKind::TagMismatch, Some(4), Some(2)));
}

#[test]
fn high_tag_numbers_round_trip() {
    // CONTEXT 40 constructed uses the high-tag-number form: 0xBF 0x28.
    let mut writer = Writer::new();
    writer.context(40, |inner| inner.write_u64(1));
    let bytes = writer.finish();
    assert_eq!(&bytes[..2], &[0xBF, 0x28]);
    let mut reader = Reader::new(&bytes);
    let element = reader.expect_context(40, true).expect("extended tag");
    assert_eq!(element.reader().expect_u64().expect("child"), 1);
}
