use core::fmt;

use super::error::{DerError, DerErrorKind, DerResult};

/// A UTC date and time decoded from UTCTime or GeneralizedTime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    /// Four-digit year.
    pub year: u16,
    /// Month `1..=12`.
    pub month: u8,
    /// Day of month `1..=31`.
    pub day: u8,
    /// Hour `0..=23`.
    pub hour: u8,
    /// Minute `0..=59`.
    pub minute: u8,
    /// Second `0..=60` (60 only for leap seconds, which DER forbids but parsing tolerates).
    pub second: u8,
}

impl DateTime {
    /// The current UTC time from the system clock.
    pub fn now_utc() -> Self {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Self::from_epoch(secs)
    }

    /// Convert Unix epoch seconds to a UTC date and time.
    pub const fn from_epoch(secs: u64) -> Self {
        let days = (secs / 86_400) as i64;
        let rest = secs % 86_400;
        let (year, month, day) = civil_from_days(days);
        Self { year, month, day, hour: (rest / 3_600) as u8, minute: ((rest % 3_600) / 60) as u8, second: (rest % 60) as u8 }
    }

    /// Decode the ASCII content of a UTCTime or GeneralizedTime element.
    pub fn parse(text: &[u8]) -> DerResult<Self> {
        let digits: Vec<u8> = text.iter().copied().filter(|byte| byte.is_ascii_digit()).collect();
        let ends_with_z = text.last().is_some_and(|byte| *byte == b'Z');
        if !ends_with_z {
            return Err(DerError::new(DerErrorKind::MalformedContent, 0));
        }
        // RFC 5280 profiles: UTCTime is YYMMDDHHMMSS (12 digits), GeneralizedTime is
        // YYYYMMDDHHMMSS (14 digits). Fractional and offset forms are rejected.
        let (year, rest) = match digits.len() {
            12 => {
                let two = number(&digits[0..2])? as u16;
                // RFC 5280 window for two-digit years.
                let year = if two >= 69 { 1900 + two } else { 2000 + two };
                (year, &digits[2..])
            }
            14 => {
                let text = core::str::from_utf8(&digits[0..4]).map_err(|_| DerError::new(DerErrorKind::MalformedContent, 0))?;
                let year = text.parse::<u16>().map_err(|_| DerError::new(DerErrorKind::MalformedContent, 0))?;
                (year, &digits[4..])
            }
            _ => return Err(DerError::new(DerErrorKind::MalformedContent, 0)),
        };
        // `rest` holds MM DD HH MM SS.
        if rest.len() != 10 {
            return Err(DerError::new(DerErrorKind::MalformedContent, 0));
        }
        let month = number(&rest[0..2])?;
        let day = number(&rest[2..4])?;
        let hour = number(&rest[4..6])?;
        let minute = number(&rest[6..8])?;
        let second = number(&rest[8..10])?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
            return Err(DerError::new(DerErrorKind::MalformedContent, 0));
        }
        Ok(Self { year, month, day, hour, minute, second })
    }

    /// The RFC 5280 UTCTime text for years `1950..2050`.
    pub fn utc_text(&self) -> String {
        format!("{:02}{:02}{:02}{:02}{:02}{:02}Z", self.year % 100, self.month, self.day, self.hour, self.minute, self.second)
    }

    /// The GeneralizedTime text with a four-digit year.
    pub fn generalized_text(&self) -> String {
        format!("{:04}{:02}{:02}{:02}{:02}{:02}Z", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Howard Hinnant's days-to-civil algorithm.
const fn civil_from_days(days: i64) -> (u16, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as u16, m as u8, d as u8)
}

fn number(bytes: &[u8]) -> DerResult<u8> {
    let text = core::str::from_utf8(bytes).map_err(|_| DerError::new(DerErrorKind::MalformedContent, 0))?;
    text.parse::<u8>().map_err(|_| DerError::new(DerErrorKind::MalformedContent, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_time_window() {
        assert_eq!(DateTime::parse(b"231130120000Z").expect("utc").year, 2023);
        assert_eq!(DateTime::parse(b"990101000000Z").expect("utc").year, 1999);
    }

    #[test]
    fn generalized_time_four_digits() {
        let value = DateTime::parse(b"20491231235959Z").expect("generalized");
        assert_eq!((value.year, value.month, value.day, value.hour, value.minute, value.second), (2049, 12, 31, 23, 59, 59));
    }

    #[test]
    fn ordering_matches_calendar() {
        let early = DateTime::parse(b"230101000000Z").expect("early");
        let late = DateTime::parse(b"231231235959Z").expect("late");
        assert!(early < late);
    }

    #[test]
    fn rejects_missing_zulu_and_bad_fields() {
        assert!(DateTime::parse(b"231130120000").is_err());
        assert!(DateTime::parse(b"231330120000Z").is_err());
        assert!(DateTime::parse(b"230132120000Z").is_err());
    }

    #[test]
    fn epoch_conversion() {
        assert_eq!(DateTime::from_epoch(0), DateTime { year: 1970, month: 1, day: 1, hour: 0, minute: 0, second: 0 });
        assert_eq!(DateTime::from_epoch(1_763_000_000).year, 2025);
        let now = DateTime::now_utc();
        assert!(now.year >= 2024);
    }

    #[test]
    fn utc_text_round_trip() {
        let value = DateTime { year: 2026, month: 11, day: 17, hour: 8, minute: 30, second: 0 };
        let text = value.utc_text();
        assert_eq!(text, "261117083000Z");
        assert_eq!(DateTime::parse(text.as_bytes()).expect("reparse"), value);
    }
}
