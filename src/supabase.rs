//! Supabase REST helpers for synchronizing training records.
//!
//! Records are posted to a Supabase PostgREST table as JSON, for example
//! `{"date":"2026-11-20","kata":"Heian Shodan","confirmed_at":"2026-11-20T08:30:00Z"}`.
//!
//! The connection is configured at build time with the `SUPABASE_HOST`,
//! `SUPABASE_ANON_KEY` and optional `SUPABASE_TABLE` environment variables;
//! see [`CONFIGURED`]. Supabase only serves HTTPS, so the plain-HTTP transport
//! in the firmware cannot reach it until TLS is added.

use crate::training::Record;
use core::fmt::Write;
use heapless::String;

/// Connection settings for a Supabase project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupabaseConfig<'a> {
    /// Project host name, e.g. `xyz.supabase.co`.
    pub host: &'a str,
    /// Public (anon) API key.
    pub api_key: &'a str,
    /// Destination table name.
    pub table: &'a str,
}

impl<'a> SupabaseConfig<'a> {
    /// Create a configuration.
    pub const fn new(host: &'a str, api_key: &'a str, table: &'a str) -> Self {
        Self {
            host,
            api_key,
            table,
        }
    }
}

/// The build-time configuration, or `None` if synchronization is disabled.
pub const CONFIGURED: Option<SupabaseConfig<'static>> =
    include!(concat!(env!("OUT_DIR"), "/supabase.rs"));

/// The formatted text does not fit in its fixed-capacity buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityExceeded;

impl From<core::fmt::Error> for CapacityExceeded {
    fn from(_: core::fmt::Error) -> Self {
        Self
    }
}

/// Serialize a `Record` as a JSON object.
pub fn format_record_json(record: &Record) -> Result<String<256>, CapacityExceeded> {
    let mut json = String::new();
    write!(json, "{{\"date\":\"{}\",\"kata\":\"", record.date)?;
    write_json_escaped(&mut json, &record.kata)?;
    match record.confirmed_at {
        Some(confirmed) => write!(json, "\",\"confirmed_at\":\"{confirmed:?}\"}}")?,
        None => write!(json, "\",\"confirmed_at\":null}}")?,
    }
    Ok(json)
}

/// Append `text` with JSON string escaping.
fn write_json_escaped<const N: usize>(out: &mut String<N>, text: &str) -> core::fmt::Result {
    for c in text.chars() {
        match c {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            c if c.is_control() => write!(out, "\\u{:04x}", u32::from(c))?,
            c => out.write_char(c)?,
        }
    }
    Ok(())
}

/// Build an HTTP/1.1 PostgREST insert request carrying `payload`.
pub fn format_post_request(
    config: &SupabaseConfig<'_>,
    payload: &str,
) -> Result<String<1024>, CapacityExceeded> {
    let mut request = String::new();
    write!(
        request,
        "POST /rest/v1/{} HTTP/1.1\r\nHost: {}\r\napikey: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nPrefer: return=minimal\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        config.table,
        config.host,
        config.api_key,
        config.api_key,
        payload.len(),
        payload
    )?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    fn record(kata: &str, confirmed: bool) -> Record {
        Record {
            date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            kata: kata.try_into().unwrap(),
            confirmed_at: confirmed.then(|| Utc.with_ymd_and_hms(2026, 11, 20, 8, 30, 0).unwrap()),
        }
    }

    #[test]
    fn formats_record_json() {
        assert_eq!(
            format_record_json(&record("Heian Shodan", true)).unwrap(),
            r#"{"date":"2026-11-20","kata":"Heian Shodan","confirmed_at":"2026-11-20T08:30:00Z"}"#
        );
        assert_eq!(
            format_record_json(&record("Tekki Shodan", false)).unwrap(),
            r#"{"date":"2026-11-20","kata":"Tekki Shodan","confirmed_at":null}"#
        );
    }

    #[test]
    fn escapes_kata_name_in_json() {
        let json = format_record_json(&record("\"Bassai\"\\\n", false)).unwrap();
        assert!(json.contains(r#""kata":"\"Bassai\"\\\u000a""#), "{json}");
    }

    #[test]
    fn formats_supabase_post_request() {
        let config = SupabaseConfig::new("xyz.supabase.co", "test-key-123", "training_logs");
        let payload = r#"{"date":"2026-11-20","kata":"Bassai Dai"}"#;
        let req = format_post_request(&config, payload).unwrap();

        assert!(req.starts_with("POST /rest/v1/training_logs HTTP/1.1\r\n"));
        assert!(req.contains("Host: xyz.supabase.co\r\n"));
        assert!(req.contains("apikey: test-key-123\r\n"));
        assert!(req.contains(&alloc::format!("Content-Length: {}\r\n", payload.len())));
        assert!(req.ends_with(payload));
    }

    #[test]
    fn reports_requests_that_do_not_fit() {
        let long_key = "k".repeat(600);
        let config = SupabaseConfig::new("xyz.supabase.co", &long_key, "training_logs");
        assert_eq!(format_post_request(&config, "{}"), Err(CapacityExceeded));
    }
}
