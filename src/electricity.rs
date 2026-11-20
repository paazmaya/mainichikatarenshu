//! Finnish spot electricity price retrieval helpers.
//!
//! Prices come from the [porssisahko.net](https://porssisahko.net/api) v2 API,
//! which reports the 15-minute (quarter-hour) price in cents per kilowatt-hour
//! (c/kWh), VAT included. `GET /v2/price.json?date=YYYY-MM-DDTHH:MM:SS.000Z`
//! returns `{"price":11.972}` for the quarter-hour containing the UTC timestamp.

use chrono::{DateTime, Datelike, Timelike, Utc};
use core::fmt::Write;
use heapless::String;
use serde::Deserialize;
use serde_json::value::RawValue;

/// Host name of the price API.
pub const PRICE_API_HOST: &str = "api.porssisahko.net";

/// Duration of one spot-price interval (15 minutes) in seconds.
const PRICE_SLOT_SECS: i64 = 15 * 60;

/// An electricity price, stored in tenths of a cent per kWh to avoid floats.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Price {
    tenths_c_kwh: i32,
}

impl Price {
    /// A price of `tenths_c_kwh / 10` c/kWh.
    pub const fn from_tenths_c_kwh(tenths_c_kwh: i32) -> Self {
        Self { tenths_c_kwh }
    }

    /// The price in tenths of a cent per kWh.
    pub const fn tenths_c_kwh(self) -> i32 {
        self.tenths_c_kwh
    }

    /// Format with one decimal, e.g. `"12.4 c/kWh"` or `"-0.5 c/kWh"`.
    pub fn format(self) -> String<16> {
        let mut text = String::new();
        let sign = if self.tenths_c_kwh < 0 { "-" } else { "" };
        let tenths = self.tenths_c_kwh.unsigned_abs();
        // Values beyond ±99 999 999.9 c/kWh do not fit and are truncated.
        let _ = write!(text, "{sign}{}.{} c/kWh", tenths / 10, tenths % 10);
        text
    }
}

/// Tracks which 15-minute UTC price slot has been delivered to the display.
#[derive(Default)]
pub struct PriceUpdates {
    last_delivered_slot: Option<i64>,
}

impl PriceUpdates {
    /// Whether the current 15-minute interval still needs a price update.
    pub fn needs_price(&self, now: DateTime<Utc>) -> bool {
        self.last_delivered_slot != Some(now.timestamp().div_euclid(PRICE_SLOT_SECS))
    }

    /// Deliver a price and mark its 15-minute slot complete only if delivery succeeds.
    pub fn deliver<E>(
        &mut self,
        now: DateTime<Utc>,
        price: Price,
        publish: impl FnOnce(String<16>) -> Result<(), E>,
    ) -> Result<(), E> {
        publish(price.format())?;
        self.last_delivered_slot = Some(now.timestamp().div_euclid(PRICE_SLOT_SECS));
        Ok(())
    }
}

/// Request path for the 15-minute price containing `now` (`ISO 8601` UTC).
pub fn price_request_path(now: DateTime<Utc>) -> String<48> {
    let mut path = String::new();
    // `/v2/price.json?date=YYYY-MM-DDTHH:MM:SS.000Z` is 44 bytes for 4-digit years.
    let _ = write!(
        path,
        "/v2/price.json?date={:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    );
    path
}

/// Extract a decimal price from a `price.json` body or full HTTP response.
///
/// Scientific notation is not supported.
pub fn parse_price_response(body: &str) -> Option<Price> {
    #[derive(Deserialize)]
    struct PriceDocument<'a> {
        #[serde(borrow)]
        price: &'a RawValue,
    }

    let body = if body.starts_with("HTTP/") {
        let response = crate::http::parse_response(body.as_bytes()).ok()?;
        if !response.is_success() {
            return None;
        }
        core::str::from_utf8(response.body).ok()?
    } else {
        body
    };
    let document: PriceDocument<'_> = serde_json::from_str(body).ok()?;
    parse_decimal_tenths(document.price.get()).map(Price::from_tenths_c_kwh)
}

/// Parse a decimal number into tenths, rounding half away from zero.
///
/// Uses integer arithmetic only: `"11.972"` → `120`, `"-1.25"` → `-13`.
fn parse_decimal_tenths(text: &str) -> Option<i32> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole: i32 = whole.parse().ok()?;
    let mut fraction_digits = fraction.bytes().map(|byte| i32::from(byte - b'0'));
    let tenths = fraction_digits.next().unwrap_or(0);
    let round_up = fraction_digits.next().unwrap_or(0) >= 5;
    let magnitude = whole
        .checked_mul(10)?
        .checked_add(tenths + i32::from(round_up))?;
    Some(if negative { -magnitude } else { magnitude })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn formats_prices_with_unit() {
        assert_eq!(Price::from_tenths_c_kwh(124).format(), "12.4 c/kWh");
        assert_eq!(Price::from_tenths_c_kwh(-5).format(), "-0.5 c/kWh");
        assert_eq!(Price::from_tenths_c_kwh(0).format(), "0.0 c/kWh");
    }

    #[test]
    fn parses_api_response_in_cents_per_kwh() {
        let price = |body| parse_price_response(body).map(Price::tenths_c_kwh);
        assert_eq!(price(r#"{"price":11.972}"#), Some(120));
        assert_eq!(price(r#"{"price": 8.94}"#), Some(89));
        assert_eq!(price(r#"{"price":-1.25}"#), Some(-13));
        assert_eq!(price(r#"{"price":-0.04}"#), Some(0));
        assert_eq!(price(r#"{"price":3}"#), Some(30));
        assert_eq!(
            price("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"price\":0.5}"),
            Some(5)
        );
    }

    #[test]
    fn rejects_missing_or_malformed_prices() {
        for body in [
            "",
            "<html>301 Moved Permanently</html>",
            r#"{"price":null}"#,
            r#"{"price":"1.5"}"#,
            r#"{"price":.5}"#,
            r#"{"price":1.2.3}"#,
            r#"{"price":1oops}"#,
            r#"{"price":1}garbage"#,
            r#"{"other":{"price":1}}"#,
            r#"{"price":1,"price":2}"#,
            "HTTP/1.1 200 OK\r\nX-Price: \"price\":1\r\n\r\n{}",
            "HTTP/1.1 500 Error\r\n\r\n{\"price\":1}",
        ] {
            assert_eq!(parse_price_response(body), None, "{body}");
        }
    }

    #[test]
    fn request_path_uses_unambiguous_utc_iso_timestamp() {
        let winter = Utc.with_ymd_and_hms(2026, 11, 20, 22, 30, 5).unwrap();
        assert_eq!(
            price_request_path(winter),
            "/v2/price.json?date=2026-11-20T22:30:05.000Z"
        );
        let summer = Utc.with_ymd_and_hms(2026, 7, 5, 4, 15, 0).unwrap();
        assert_eq!(
            price_request_path(summer),
            "/v2/price.json?date=2026-07-05T04:15:00.000Z"
        );
    }

    #[test]
    fn failed_delivery_remains_due_until_published() {
        let now = Utc.with_ymd_and_hms(2026, 11, 20, 10, 0, 0).unwrap();
        let same_quarter = Utc.with_ymd_and_hms(2026, 11, 20, 10, 14, 59).unwrap();
        let next_quarter = Utc.with_ymd_and_hms(2026, 11, 20, 10, 15, 0).unwrap();
        let mut updates = PriceUpdates::default();
        let price = Price::from_tenths_c_kwh(123);
        assert!(updates.needs_price(now));
        assert_eq!(updates.deliver(now, price, |_| Err("full")), Err("full"));
        assert!(updates.needs_price(now));
        updates
            .deliver(now, price, |text| {
                assert_eq!(text, "12.3 c/kWh");
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(!updates.needs_price(same_quarter));
        assert!(updates.needs_price(next_quarter));
    }

    #[test]
    fn repeated_helsinki_hour_uses_distinct_utc_paths_and_slots() {
        let first = Utc.with_ymd_and_hms(2026, 10, 25, 0, 30, 0).unwrap();
        let second = Utc.with_ymd_and_hms(2026, 10, 25, 1, 30, 0).unwrap();
        assert_ne!(price_request_path(first), price_request_path(second));
        let mut updates = PriceUpdates::default();
        updates
            .deliver(first, Price::from_tenths_c_kwh(1), |_| Ok::<_, ()>(()))
            .unwrap();
        assert!(updates.needs_price(second));
    }
}
