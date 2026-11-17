use crate::Oid;

fn oid(text: &str) -> Oid {
    Oid::from_dotted(text).expect("static OID parses")
}

/// `2.5.4.3` commonName.
pub fn common_name() -> Oid {
    oid("2.5.4.3")
}

/// `2.5.4.6` countryName.
pub fn country_name() -> Oid {
    oid("2.5.4.6")
}

/// `2.5.4.10` organizationName.
pub fn organization_name() -> Oid {
    oid("2.5.4.10")
}

/// `1.2.840.10045.2.1` id-ecPublicKey.
pub fn ec_public_key() -> Oid {
    oid("1.2.840.10045.2.1")
}

/// `1.2.840.10045.3.1.7` prime256v1 (P-256).
pub fn prime256v1() -> Oid {
    oid("1.2.840.10045.3.1.7")
}

/// `1.3.132.0.34` secp384r1 (P-384).
pub fn secp384r1() -> Oid {
    oid("1.3.132.0.34")
}

/// `1.2.840.113549.1.1.1` rsaEncryption.
pub fn rsa_encryption() -> Oid {
    oid("1.2.840.113549.1.1.1")
}

/// `1.2.840.113549.1.1.10` RSASSA-PSS.
pub fn rsa_pss() -> Oid {
    oid("1.2.840.113549.1.1.10")
}

/// `1.2.840.113549.1.1.11` sha256WithRSAEncryption.
pub fn sha256_with_rsa() -> Oid {
    oid("1.2.840.113549.1.1.11")
}

/// `1.2.840.113549.1.1.12` sha384WithRSAEncryption.
pub fn sha384_with_rsa() -> Oid {
    oid("1.2.840.113549.1.1.12")
}

/// `2.16.840.1.101.3.4.2.1` sha256 with ECDSA.
pub fn sha256() -> Oid {
    oid("2.16.840.1.101.3.4.2.1")
}

/// `2.16.840.1.101.3.4.2.2` sha384.
pub fn sha384() -> Oid {
    oid("2.16.840.1.101.3.4.2.2")
}

/// `1.2.840.10045.4.3.2` ecdsa-with-SHA256.
pub fn ecdsa_with_sha256() -> Oid {
    oid("1.2.840.10045.4.3.2")
}

/// `1.2.840.10045.4.3.3` ecdsa-with-SHA384.
pub fn ecdsa_with_sha384() -> Oid {
    oid("1.2.840.10045.4.3.3")
}

/// `2.5.29.14` subjectKeyIdentifier.
pub fn subject_key_identifier() -> Oid {
    oid("2.5.29.14")
}

/// `2.5.29.15` keyUsage.
pub fn key_usage() -> Oid {
    oid("2.5.29.15")
}

/// `2.5.29.17` subjectAltName.
pub fn subject_alt_name() -> Oid {
    oid("2.5.29.17")
}

/// `2.5.29.19` basicConstraints.
pub fn basic_constraints() -> Oid {
    oid("2.5.29.19")
}

/// `2.5.29.35` authorityKeyIdentifier.
pub fn authority_key_identifier() -> Oid {
    oid("2.5.29.35")
}

/// `2.5.29.37` extendedKeyUsage.
pub fn extended_key_usage() -> Oid {
    oid("2.5.29.37")
}

/// `1.2.840.113549.1.9.14` extensionRequest (CSR attribute).
pub fn extension_request() -> Oid {
    oid("1.2.840.113549.1.9.14")
}

/// `1.3.6.1.5.5.7.3.1` id-kp-serverAuth.
pub fn server_auth() -> Oid {
    oid("1.3.6.1.5.5.7.3.1")
}

/// `1.3.6.1.5.5.7.3.2` id-kp-clientAuth.
pub fn client_auth() -> Oid {
    oid("1.3.6.1.5.5.7.3.2")
}

/// `1.3.6.1.5.5.7.48.1.1` id-ad-ocsp.
pub fn ad_ocsp() -> Oid {
    oid("1.3.6.1.5.5.7.48.1.1")
}

/// `1.3.6.1.5.5.7.48.2` id-ad-caIssuers.
pub fn ad_ca_issuers() -> Oid {
    oid("1.3.6.1.5.5.7.48.2")
}

/// `1.2.840.113549.1.5.13` PBES2 (encrypted PEM export).
pub fn pbes2() -> Oid {
    oid("1.2.840.113549.1.5.13")
}

/// `1.2.840.113549.1.5.12` PBKDF2.
pub fn pbkdf2() -> Oid {
    oid("1.2.840.113549.1.5.12")
}

/// `1.2.840.113549.1.5.11` PBES1 with sha1 and des-ede3-cbc.
pub fn pbe_with_sha1_des3() -> Oid {
    oid("1.2.840.113549.1.5.11")
}

/// `2.16.840.1.101.3.4.1.2` aes-128-cbc.
pub fn aes128_cbc() -> Oid {
    oid("2.16.840.1.101.3.4.1.2")
}

/// `2.16.840.1.101.3.4.1.42` aes-256-cbc.
pub fn aes256_cbc() -> Oid {
    oid("2.16.840.1.101.3.4.1.42")
}

/// `2.16.840.1.101.3.4.1.46` aes-256-gcm.
pub fn aes256_gcm() -> Oid {
    oid("2.16.840.1.101.3.4.1.46")
}

/// `1.2.840.113549.1.1.8` id-MGF1.
pub fn mgf1() -> Oid {
    oid("1.2.840.113549.1.1.8")
}
