use core::fmt;

/// The four ASN.1 tag classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Standard types defined by X.680.
    Universal,
    /// Types defined by an application.
    Application,
    /// Types bound to a specific place in a structure.
    Context,
    /// Reserved for private use.
    Private,
}

impl Class {
    /// The two-bit class number.
    pub const fn bits(self) -> u8 {
        match self {
            Self::Universal => 0b00,
            Self::Application => 0b01,
            Self::Context => 0b10,
            Self::Private => 0b11,
        }
    }

    /// Decode the two class bits from a first tag byte.
    pub const fn from_bits(bits: u8) -> Self {
        match bits >> 6 {
            0b00 => Self::Universal,
            0b01 => Self::Application,
            0b10 => Self::Context,
            _ => Self::Private,
        }
    }
}

/// A complete ASN.1 tag: class, constructed bit, and tag number.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    /// The tag class.
    pub class: Class,
    /// Whether the value is constructed.
    pub constructed: bool,
    /// The tag number.
    pub number: u32,
}

impl Tag {
    /// Build a universal tag.
    pub const fn universal(number: u32) -> Self {
        Self { class: Class::Universal, constructed: false, number }
    }

    /// Build a universal constructed tag.
    pub const fn universal_constructed(number: u32) -> Self {
        Self { class: Class::Universal, constructed: true, number }
    }

    /// Build a context-specific tag.
    pub const fn context(number: u32, constructed: bool) -> Self {
        Self { class: Class::Context, constructed, number }
    }

    /// BOOLEAN (`0x01`).
    pub const BOOLEAN: Self = Self::universal(0x01);
    /// INTEGER (`0x02`).
    pub const INTEGER: Self = Self::universal(0x02);
    /// BIT STRING (`0x03`).
    pub const BIT_STRING: Self = Self::universal(0x03);
    /// OCTET STRING (`0x04`).
    pub const OCTET_STRING: Self = Self::universal(0x04);
    /// NULL (`0x05`).
    pub const NULL: Self = Self::universal(0x05);
    /// OBJECT IDENTIFIER (`0x06`).
    pub const OID: Self = Self::universal(0x06);
    /// UTF8String (`0x0C`).
    pub const UTF8_STRING: Self = Self::universal(0x0C);
    /// PrintableString (`0x13`).
    pub const PRINTABLE_STRING: Self = Self::universal(0x13);
    /// IA5String (`0x16`).
    pub const IA5_STRING: Self = Self::universal(0x16);
    /// UTCTime (`0x17`).
    pub const UTCTIME: Self = Self::universal(0x17);
    /// GeneralizedTime (`0x18`).
    pub const GENERALIZED_TIME: Self = Self::universal(0x18);
    /// SEQUENCE (number 16, first byte `0x30`).
    pub const SEQUENCE: Self = Self::universal_constructed(16);
    /// SET (number 17, first byte `0x31`).
    pub const SET: Self = Self::universal_constructed(17);

    /// The first tag byte when the number fits in five bits, else the lead byte of the high-tag-number form.
    pub const fn lead_byte(self) -> u8 {
        let constructed = if self.constructed { 0b0010_0000 } else { 0 };
        let class = self.class.bits() << 6;
        if self.number < 31 { class | constructed | self.number as u8 } else { class | constructed | 0b0001_1111 }
    }

    /// The tag number encoded in the base-128 high-tag-number continuation bytes.
    pub const fn needs_extended_number(self) -> bool {
        self.number >= 31
    }

    /// Compare by class and number, ignoring the constructed bit.
    pub const fn same_number(self, other: Self) -> bool {
        self.number == other.number && self.class.bits() == other.class.bits()
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Tag({self})")
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = match self.class {
            Class::Universal => "UNIVERSAL",
            Class::Application => "APPLICATION",
            Class::Context => "CONTEXT",
            Class::Private => "PRIVATE",
        };
        let constructed = if self.constructed { ", constructed" } else { "" };
        write!(formatter, "{class} {}{constructed}", self.number)
    }
}
