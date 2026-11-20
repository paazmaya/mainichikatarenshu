//! Supabase REST helpers and synchronization state for training records.
//!
//! Records are posted to a Supabase PostgREST table over HTTPS as JSON, for example
//! `{"date":"2026-11-20","kata":"Heian Shodan","confirmed_at":"2026-11-20T08:30:00Z"}`.
//!
//! The connection is configured at build time with the `SUPABASE_HOST`,
//! `SUPABASE_ANON_KEY` and optional `SUPABASE_TABLE` environment variables;
//! see [`CONFIGURED`].
//!
//! Delivery is at-least-once: a record stays in the local pending queue until a
//! complete `2xx` HTTP response is received and delivered back to the main loop.
//! If the server stores the row and the connection drops before the response
//! arrives, the firmware will retry the same date later; configure a `UNIQUE`
//! constraint on `date` (or an idempotent policy) in Supabase if duplicates are
//! undesirable.

use crate::training::Record;
use chrono::NaiveDate;
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

/// Result of uploading one pending training record to Supabase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncOutcome {
    /// The record for this date received a complete 2xx response and may be acknowledged.
    Synced(NaiveDate),
    /// The upload failed; the record remains pending.
    Failed,
}

/// Main-loop state tracking at most one in-flight record upload at a time.
#[derive(Debug, Default)]
pub struct RecordSync {
    in_flight: Option<NaiveDate>,
}

impl RecordSync {
    /// Whether an upload request is currently awaiting an outcome.
    pub const fn is_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Process a completed upload outcome; returns the date to acknowledge, if any.
    pub fn complete(&mut self, outcome: SyncOutcome) -> Option<NaiveDate> {
        let expected = self.in_flight.take();
        match outcome {
            SyncOutcome::Synced(date) if expected == Some(date) => Some(date),
            SyncOutcome::Synced(_) | SyncOutcome::Failed => None,
        }
    }

    /// Queue the oldest pending record when Supabase is configured, the clock is valid,
    /// and no upload is already in flight.
    pub fn dispatch_next<E>(
        &mut self,
        configured: bool,
        clock_valid: bool,
        pending: &[Record],
        send: impl FnOnce(Record) -> Result<(), E>,
    ) -> Result<bool, E> {
        if !configured || !clock_valid || self.in_flight.is_some() {
            return Ok(false);
        }
        let Some(oldest) = pending.first() else {
            return Ok(false);
        };
        send(oldest.clone())?;
        self.in_flight = Some(oldest.date);
        Ok(true)
    }
}

/// Network-task buffer ensuring [`SyncOutcome`] is never dropped when the result channel is full.
#[derive(Debug, Default)]
pub struct OutcomeDelivery {
    pending: Option<SyncOutcome>,
}

impl OutcomeDelivery {
    /// Whether a completed upload outcome is still waiting to be published.
    pub const fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Stage `outcome` for delivery to the main loop.
    pub fn stage(&mut self, outcome: SyncOutcome) {
        self.pending = Some(outcome);
    }

    /// Attempt to publish any staged outcome, retaining it on error for a later retry.
    pub fn flush<E>(
        &mut self,
        publish: impl FnOnce(SyncOutcome) -> Result<(), E>,
    ) -> Result<Option<SyncOutcome>, E> {
        let Some(outcome) = self.pending else {
            return Ok(None);
        };
        publish(outcome)?;
        self.pending = None;
        Ok(Some(outcome))
    }
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

    #[test]
    fn record_sync_waits_for_clock_and_preserves_pending_on_failure() {
        let first = record("Heian Shodan", true);
        let mut second = record("Heian Nidan", false);
        second.date = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        let queue = [first.clone(), second.clone()];

        let mut sync = RecordSync::default();
        // Disabled or unsynchronized clock will not dispatch.
        assert_eq!(
            sync.dispatch_next(false, true, &queue, |_| Ok::<_, &str>(())),
            Ok(false)
        );
        assert_eq!(
            sync.dispatch_next(true, false, &queue, |_| Ok::<_, &str>(())),
            Ok(false)
        );

        // Full request channel reports error and leaves `in_flight` unset so the next poll retries.
        assert_eq!(
            sync.dispatch_next(true, true, &queue, |_| Err("full")),
            Err("full")
        );
        assert!(!sync.is_in_flight());

        // Successful dispatch locks onto the oldest date and suppresses duplicate dispatch.
        let mut sent = None;
        assert_eq!(
            sync.dispatch_next(true, true, &queue, |rec| {
                sent = Some(rec.date);
                Ok::<_, ()>(())
            }),
            Ok(true)
        );
        assert_eq!(sent, Some(first.date));
        assert!(sync.is_in_flight());
        assert_eq!(
            sync.dispatch_next(true, true, &queue, |_| Ok::<_, ()>(())),
            Ok(false)
        );

        // Failed upload clears in-flight without acknowledging the record.
        assert_eq!(sync.complete(SyncOutcome::Failed), None);
        assert!(!sync.is_in_flight());

        // Re-dispatch and complete with matching date acknowledges only that date.
        sync.dispatch_next(true, true, &queue, |_| Ok::<_, ()>(()))
            .unwrap();
        assert_eq!(sync.complete(SyncOutcome::Synced(second.date)), None);
        sync.dispatch_next(true, true, &queue, |_| Ok::<_, ()>(()))
            .unwrap();
        assert_eq!(
            sync.complete(SyncOutcome::Synced(first.date)),
            Some(first.date)
        );
    }

    #[test]
    fn outcome_delivery_retains_result_across_channel_saturation() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let mut delivery = OutcomeDelivery::default();
        assert!(!delivery.has_pending());
        assert_eq!(delivery.flush(|_| Ok::<_, &str>(())), Ok(None));

        delivery.stage(SyncOutcome::Synced(date));
        assert!(delivery.has_pending());
        assert_eq!(delivery.flush(|_| Err("full")), Err("full"));
        assert!(delivery.has_pending());

        let mut received = None;
        assert_eq!(
            delivery.flush(|outcome| {
                received = Some(outcome);
                Ok::<_, ()>(())
            }),
            Ok(Some(SyncOutcome::Synced(date)))
        );
        assert_eq!(received, Some(SyncOutcome::Synced(date)));
        assert!(!delivery.has_pending());
    }
}
