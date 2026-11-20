//! Spot electricity price parsing and formatting.
//!
//! Provides parsing and representation of hourly spot prices (e.g. from Nord Pool / ENTSO-E).
//! Standard electricity prices in the Finnish/Nordic market are commonly quoted in
//! cents per kilowatt-hour (c/kWh) or EUR/MWh (1 EUR/MWh = 0.1 c/kWh).

use heapless::String;

/// Represents an hourly spot price entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceEntry {
    /// UTC timestamp of the hour start.
    pub timestamp: i64,
    /// Price in tenths of a cent per kWh (e.g. 124 represents 12.4 c/kWh).
    pub price_tenths_c_kwh: i32,
}

impl PriceEntry {
    pub const fn new(timestamp: i64, price_tenths_c_kwh: i32) -> Self {
        Self {
            timestamp,
            price_tenths_c_kwh,
        }
    }

    /// Converts EUR per MWh to tenths of a cent per kWh.
    /// 1 EUR/MWh = 0.100 c/kWh = 1 tenth of a cent.
    pub fn from_eur_mwh(timestamp: i64, eur_mwh: f32) -> Self {
        let price_tenths = (eur_mwh + 0.5) as i32;
        Self {
            timestamp,
            price_tenths_c_kwh: price_tenths,
        }
    }

    /// Formats the price as a human-readable string, e.g. "12.4 c/kWh".
    pub fn format_price(&self) -> String<16> {
        let mut s = String::new();
        let sign = if self.price_tenths_c_kwh < 0 { "-" } else { "" };
        let abs_val = self.price_tenths_c_kwh.abs();
        let whole = abs_val / 10;
        let frac = abs_val % 10;

        use core::fmt::Write;
        let _ = write!(s, "{}{}.{} c", sign, whole, frac);
        s
    }
}

/// Parses a simple JSON response containing a list of prices.
///
/// Example JSON shape supported (from public spot price APIs):
/// `{"prices":[{"startDate":"2026-11-20T10:00:00.000Z","price":12.35}]}`
/// or `[{"price": 12.4, "startDate": "..."}]`
///
/// Extracts the first or current price found for minimal memory footprint.
pub fn parse_first_price_eur_mwh(json: &str) -> Option<f32> {
    // Locate "price" key
    let idx = json.find("\"price\"")?;
    let rest = &json[idx + 7..];
    let start = rest.find(|c: char| c.is_ascii_digit() || c == '-' || c == '.')?;
    let number_str = &rest[start..];
    let end = number_str
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(number_str.len());

    parse_float(&number_str[..end])
}

fn parse_float(s: &str) -> Option<f32> {
    // Simple no_std float parsing
    let mut parts = s.split('.');
    let whole_str = parts.next()?;
    let is_neg = whole_str.starts_with('-');
    let whole_digits = if is_neg { &whole_str[1..] } else { whole_str };

    let whole: i32 = whole_digits.parse().ok()?;
    let frac = match parts.next() {
        Some(frac_str) if !frac_str.is_empty() => {
            let digits = frac_str.as_bytes();
            let mut val = 0.0f32;
            let mut divisor = 10.0f32;
            for &d in digits.iter().take(4) {
                if !d.is_ascii_digit() {
                    break;
                }
                val += (d - b'0') as f32 / divisor;
                divisor *= 10.0;
            }
            val
        }
        _ => 0.0,
    };

    let result = (whole as f32) + frac;
    Some(if is_neg { -result } else { result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_positive_and_negative_prices() {
        let p1 = PriceEntry::new(1795168800, 124);
        assert_eq!(p1.format_price(), "12.4 c");

        let p2 = PriceEntry::new(1795168800, -5);
        assert_eq!(p2.format_price(), "-0.5 c");

        let p3 = PriceEntry::new(1795168800, 0);
        assert_eq!(p3.format_price(), "0.0 c");
    }

    #[test]
    fn parses_json_price() {
        let json = r#"{"prices":[{"price":89.5,"startDate":"2026-11-20T10:00:00Z"}]}"#;
        let price = parse_first_price_eur_mwh(json);
        assert!(price.is_some());
        let val = price.unwrap();
        assert!((val - 89.5).abs() < 0.1);

        let json_neg = r#"{"price": -1.25}"#;
        let price_neg = parse_first_price_eur_mwh(json_neg);
        assert!(price_neg.is_some());
        let val_neg = price_neg.unwrap();
        assert!((val_neg - -1.25).abs() < 0.1);
    }
}
