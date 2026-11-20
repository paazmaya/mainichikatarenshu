//! Validation of wall-clock time from the RTC, serial input or SNTP.
//!
//! The RTC counts microseconds since the Unix epoch. A value before
//! [`MIN_VALID_UNIX_SECS`] means the clock was never set.

use chrono::{DateTime, Utc};

/// 2000-01-01T00:00:00Z; earlier timestamps are treated as an unset clock.
pub const MIN_VALID_UNIX_SECS: u64 = 946_684_800;

const MICROS_PER_SEC: u64 = 1_000_000;

/// Convert trusted Unix seconds to RTC microseconds.
///
/// Returns `None` if the time is before [`MIN_VALID_UNIX_SECS`], overflows or
/// cannot be represented as a [`DateTime`].
pub fn rtc_micros_from_unix_secs(unix_secs: u64) -> Option<u64> {
    if unix_secs < MIN_VALID_UNIX_SECS {
        return None;
    }
    DateTime::<Utc>::from_timestamp(i64::try_from(unix_secs).ok()?, 0)?;
    unix_secs.checked_mul(MICROS_PER_SEC)
}

/// Parse a decimal Unix timestamp in seconds into RTC microseconds.
pub fn parse_unix_secs(text: &str) -> Option<u64> {
    rtc_micros_from_unix_secs(text.trim().parse().ok()?)
}

/// Whether an RTC reading (microseconds) holds a time that was set.
pub fn is_rtc_time_valid(rtc_micros: u64) -> bool {
    rtc_micros / MICROS_PER_SEC >= MIN_VALID_UNIX_SECS
}

/// Whole Unix seconds of an RTC reading.
pub fn unix_secs_from_rtc_micros(rtc_micros: u64) -> u64 {
    rtc_micros / MICROS_PER_SEC
}

/// UTC time of an RTC reading, truncated to whole seconds.
pub fn utc_from_rtc_micros(rtc_micros: u64) -> Option<DateTime<Utc>> {
    let secs = i64::try_from(unix_secs_from_rtc_micros(rtc_micros)).ok()?;
    DateTime::<Utc>::from_timestamp(secs, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_times_from_2000_onwards() {
        assert_eq!(
            rtc_micros_from_unix_secs(MIN_VALID_UNIX_SECS),
            Some(MIN_VALID_UNIX_SECS * MICROS_PER_SEC)
        );
        assert_eq!(rtc_micros_from_unix_secs(MIN_VALID_UNIX_SECS - 1), None);
        assert_eq!(rtc_micros_from_unix_secs(u64::MAX), None);
    }

    #[test]
    fn parses_serial_input() {
        assert_eq!(parse_unix_secs("1767225600"), Some(1_767_225_600_000_000));
        assert_eq!(parse_unix_secs(" 1767225600 "), Some(1_767_225_600_000_000));
        for invalid in ["", "-1", "12", "1767225600.5", "now"] {
            assert_eq!(parse_unix_secs(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn interprets_rtc_readings() {
        let micros = 1_767_225_600_123_456;
        assert!(is_rtc_time_valid(micros));
        assert!(!is_rtc_time_valid(5_000_000));
        assert_eq!(
            utc_from_rtc_micros(micros),
            DateTime::<Utc>::from_timestamp(1_767_225_600, 0)
        );
    }
}
