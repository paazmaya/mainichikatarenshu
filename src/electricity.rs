//! Finnish spot electricity price retrieval helpers.
//!
//! Prices come from the [porssisahko.net](https://porssisahko.net/api) API,
//! which reports the hourly price in cents per kilowatt-hour (c/kWh), VAT
//! included. `GET /v1/price.json?date=YYYY-MM-DD&hour=H` returns `{"price":11.972}`
//! for the given Helsinki-local hour.
//!
//! The API only serves HTTPS; plain HTTP requests are redirected.

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Europe::Helsinki;
use core::fmt::Write;
use heapless::String;

/// Host name of the price API.
pub const PRICE_API_HOST: &str = "api.porssisahko.net";

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

/// Request path for the price of the Helsinki-local hour containing `now`.
pub fn price_request_path(now: DateTime<Utc>) -> String<48> {
    let local = now.with_timezone(&Helsinki);
    let mut path = String::new();
    // The longest path is 39 bytes, so this cannot overflow.
    let _ = write!(
        path,
        "/v1/price.json?date={}&hour={}",
        local.date_naive(),
        local.hour()
    );
    path
}

/// Extract the price from a `price.json` response body or full HTTP response.
pub fn parse_price_response(body: &str) -> Option<Price> {
    let after_key = &body[body.find("\"price\"")? + "\"price\"".len()..];
    let value = after_key.trim_start().strip_prefix(':')?.trim_start();
    let end = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(value.len());
    parse_decimal_tenths(&value[..end]).map(Price::from_tenths_c_kwh)
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
        ] {
            assert_eq!(parse_price_response(body), None, "{body}");
        }
    }

    #[test]
    fn request_path_uses_helsinki_hour() {
        let winter = Utc.with_ymd_and_hms(2026, 11, 20, 22, 30, 0).unwrap();
        assert_eq!(
            price_request_path(winter),
            "/v1/price.json?date=2026-11-21&hour=0"
        );
        let summer = Utc.with_ymd_and_hms(2026, 7, 5, 4, 0, 0).unwrap();
        assert_eq!(
            price_request_path(summer),
            "/v1/price.json?date=2026-07-05&hour=7"
        );
    }
}
