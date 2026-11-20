//! Supabase REST client utilities for synchronizing training records.
//!
//! Posts completed daily training records to the Supabase PostgREST table.
//! Serializes records as JSON:
//! `{"date":"2026-11-20","kata":"Heian Shodan","confirmed_at":"2026-11-20T08:30:00Z"}`

use crate::training::Record;
use core::fmt::Write;
use heapless::String;

/// Configuration for connecting to Supabase.
#[derive(Clone, Debug)]
pub struct SupabaseConfig<'a> {
    pub host: &'a str,
    pub api_key: &'a str,
    pub table: &'a str,
}

impl<'a> SupabaseConfig<'a> {
    pub const fn new(host: &'a str, api_key: &'a str, table: &'a str) -> Self {
        Self {
            host,
            api_key,
            table,
        }
    }
}

/// Formats a single `Record` into a JSON string payload.
pub fn format_record_json(record: &Record) -> String<256> {
    let mut json = String::new();
    let _ = write!(json, "{{\"date\":\"{}\",\"kata\":\"", record.date);

    // Escape any quotes in kata name if present
    for c in record.kata.chars() {
        if c == '"' || c == '\\' {
            let _ = json.push('\\');
        }
        let _ = json.push(c);
    }

    if let Some(confirmed) = record.confirmed_at {
        let _ = write!(json, "\",\"confirmed_at\":\"{:?}\"}}", confirmed);
    } else {
        let _ = write!(json, "\",\"confirmed_at\":null}}");
    }

    json
}

/// Creates a standard HTTP POST request header for Supabase PostgREST insert.
pub fn format_post_request(config: &SupabaseConfig<'_>, payload: &str) -> String<512> {
    let mut req = String::new();
    let _ = write!(
        req,
        "POST /rest/v1/{} HTTP/1.1\r\nHost: {}\r\napikey: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nPrefer: return=minimal\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        config.table,
        config.host,
        config.api_key,
        config.api_key,
        payload.len(),
        payload
    );
    req
}

/// Checks if an HTTP response status is 200, 201 Created, or 204 No Content.
pub fn is_http_success(response: &str) -> bool {
    response.starts_with("HTTP/1.1 200")
        || response.starts_with("HTTP/1.1 201")
        || response.starts_with("HTTP/1.1 204")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    #[test]
    fn formats_record_json_with_confirmation() {
        let record = Record {
            date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            kata: "Heian Shodan".try_into().unwrap(),
            confirmed_at: Some(Utc.with_ymd_and_hms(2026, 11, 20, 8, 30, 0).unwrap()),
        };

        let json = format_record_json(&record);
        assert!(json.contains("\"date\":\"2026-11-20\""));
        assert!(json.contains("\"kata\":\"Heian Shodan\""));
        assert!(json.contains("\"confirmed_at\":\"2026-11-20T08:30:00Z\""));
    }

    #[test]
    fn formats_record_json_unconfirmed() {
        let record = Record {
            date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            kata: "Tekki Shodan".try_into().unwrap(),
            confirmed_at: None,
        };

        let json = format_record_json(&record);
        assert!(json.contains("\"date\":\"2026-11-20\""));
        assert!(json.contains("\"kata\":\"Tekki Shodan\""));
        assert!(json.contains("\"confirmed_at\":null"));
    }

    #[test]
    fn formats_supabase_post_request() {
        let config = SupabaseConfig::new("xyz.supabase.co", "test-key-123", "training_logs");
        let payload = r#"{"date":"2026-11-20","kata":"Bassai Dai"}"#;
        let req = format_post_request(&config, payload);

        assert!(req.starts_with("POST /rest/v1/training_logs HTTP/1.1\r\n"));
        assert!(req.contains("Host: xyz.supabase.co\r\n"));
        assert!(req.contains("apikey: test-key-123\r\n"));
        assert!(req.contains(&format!("Content-Length: {}\r\n", payload.len())));
        assert!(req.ends_with(payload));
    }

    #[test]
    fn verifies_http_success() {
        assert!(is_http_success("HTTP/1.1 201 Created\r\n..."));
        assert!(is_http_success("HTTP/1.1 204 No Content\r\n..."));
        assert!(is_http_success("HTTP/1.1 200 OK\r\n..."));
        assert!(!is_http_success("HTTP/1.1 400 Bad Request\r\n..."));
        assert!(!is_http_success(
            "HTTP/1.1 500 Internal Server Error\r\n..."
        ));
    }
}
