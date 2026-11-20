//! Authenticated TLS 1.2/1.3 client support (`wifi` and `test-utils` features).
//!
//! Server certificates are verified against an explicit X.509 CA bundle with
//! `AuthMode::Required`, SNI and DNS SAN hostname verification, and certificate
//! validity dates (`notBefore` / `notAfter`) checked against a validated UTC
//! [`WallClock`]. When the wall clock is unset, verification fails closed.

use crate::{
    clock,
    http::{self, ReadResponseError, Response},
};
use chrono::{DateTime, Datelike, Timelike, Utc};
use core::{cell::Cell, ffi::CStr};
use critical_section::Mutex;
use embedded_io_async::{Read, Write};
use mbedtls_rs::{
    sys::{
        hook::{timer::MbedtlsTimer, wall_clock::MbedtlsWallClock},
        tm, MbedtlsError,
    },
    AuthMode, Certificate, ClientSessionConfig, Session, SessionConfig, SessionError, TlsReference,
    TlsVersion, X509,
};

include!(concat!(env!("OUT_DIR"), "/ca_bundle.rs"));

/// Standard HTTPS TCP port.
pub const HTTPS_PORT: u16 = 443;

const MAX_HOST_LEN: usize = 253;
const ALPN_HTTP11: &[&CStr] = &[c"http/1.1"];

/// Thread-safe UTC wall-clock anchor paired with a monotonic millisecond timer.
///
/// Stores `(unix_secs, monotonic_ms)` only for timestamps at or after
/// [`clock::MIN_VALID_UNIX_SECS`]. Until set, [`Self::now`] returns `None`.
pub struct WallClock {
    anchor: Mutex<Cell<Option<(u64, u64)>>>,
}

impl Default for WallClock {
    fn default() -> Self {
        Self::new()
    }
}

impl WallClock {
    /// Create an unset wall clock.
    pub const fn new() -> Self {
        Self {
            anchor: Mutex::new(Cell::new(None)),
        }
    }

    /// Record `unix_secs` observed at monotonic `now_ms`.
    ///
    /// Rejects timestamps earlier than [`clock::MIN_VALID_UNIX_SECS`] or out of range,
    /// leaving the previous anchor unchanged.
    pub fn set_unix_secs(&self, unix_secs: u64, now_ms: u64) -> bool {
        if clock::rtc_micros_from_unix_secs(unix_secs).is_none() {
            return false;
        }
        critical_section::with(|cs| {
            self.anchor.borrow(cs).set(Some((unix_secs, now_ms)));
        });
        true
    }

    /// Clear any recorded time so subsequent queries fail closed.
    pub fn clear(&self) {
        critical_section::with(|cs| {
            self.anchor.borrow(cs).set(None);
        });
    }

    /// Whether a valid wall-clock anchor is currently available at `now_ms`.
    pub fn is_valid(&self, now_ms: u64) -> bool {
        self.now(now_ms).is_some()
    }

    /// Current UTC timestamp derived from the anchor and `now_ms`.
    pub fn now(&self, now_ms: u64) -> Option<DateTime<Utc>> {
        let (base_secs, base_ms) = critical_section::with(|cs| self.anchor.borrow(cs).get())?;
        let elapsed_secs = now_ms.checked_sub(base_ms)? / 1000;
        let current_secs = base_secs.checked_add(elapsed_secs)?;
        clock::rtc_micros_from_unix_secs(current_secs)?;
        DateTime::from_timestamp(i64::try_from(current_secs).ok()?, 0)
    }
}

/// Bridges [`WallClock`] and an [`MbedtlsTimer`] into [`MbedtlsWallClock`] for X.509 date checks.
pub struct MbedtlsClock<'a> {
    clock: &'a WallClock,
    timer: &'a (dyn MbedtlsTimer + Send + Sync),
}

impl<'a> MbedtlsClock<'a> {
    /// Pair `clock` with monotonic `timer`.
    pub const fn new(clock: &'a WallClock, timer: &'a (dyn MbedtlsTimer + Send + Sync)) -> Self {
        Self { clock, timer }
    }
}

impl MbedtlsWallClock for MbedtlsClock<'_> {
    fn instant(&self) -> Option<tm> {
        let now_ms = u64::try_from(self.timer.now()).ok()?;
        let utc = self.clock.now(now_ms)?;
        Some(utc_to_tm(utc))
    }
}

fn utc_to_tm(utc: DateTime<Utc>) -> tm {
    // Zero-initialize platform-specific extension fields (`tm_gmtoff`, `tm_zone`)
    // when building on host targets while setting all standard C99 UTC fields.
    let mut out: tm = unsafe { core::mem::zeroed() };
    out.tm_sec = utc.second() as i32;
    out.tm_min = utc.minute() as i32;
    out.tm_hour = utc.hour() as i32;
    out.tm_mday = utc.day() as i32;
    out.tm_mon = utc.month0() as i32;
    out.tm_year = utc.year() - 1900;
    out.tm_wday = utc.weekday().num_days_from_sunday() as i32;
    out.tm_yday = utc.ordinal0() as i32;
    out.tm_isdst = 0;
    out
}

/// Install the global MbedTLS timer and wall-clock hooks.
///
/// # Safety
/// Must be called before X.509 certificate verification runs and while no
/// concurrent MbedTLS session is reading the hooks.
pub unsafe fn install_hooks(
    timer: &'static (dyn MbedtlsTimer + Send + Sync),
    wall_clock: &'static (dyn MbedtlsWallClock + Send + Sync),
) {
    unsafe {
        mbedtls_rs::sys::hook::timer::hook_timer(Some(timer));
        mbedtls_rs::sys::hook::wall_clock::hook_wall_clock(Some(wall_clock));
    }
}

/// Errors returned while preparing or executing an authenticated HTTPS exchange.
#[derive(Debug)]
pub enum ExchangeError {
    /// Wall-clock time has not been synchronized yet; certificate dates cannot be verified.
    ClockNotSynchronized,
    /// Target host name is empty, too long, an IP literal, or contains invalid characters.
    InvalidServerName,
    /// Trusted CA bundle could not be parsed.
    InvalidCaBundle(MbedtlsError),
    /// TLS handshake, certificate verification or record transport failed.
    Tls(SessionError),
    /// Peer certificate verification left non-zero failure flags.
    CertificateVerificationFailed,
    /// HTTP response read or framing validation failed.
    Response(ReadResponseError<SessionError>),
}

/// Parse the embedded default WebPKI root CA bundle (`certs/ca-roots.pem` or `TLS_CA_BUNDLE`).
pub fn load_default_ca_bundle() -> Result<Certificate<'static>, ExchangeError> {
    parse_ca_pem(CA_BUNDLE_PEM)
}

/// Parse a nul-terminated PEM bundle of trusted root CAs.
pub fn parse_ca_pem(pem: &CStr) -> Result<Certificate<'static>, ExchangeError> {
    Certificate::new(X509::PEM(pem)).map_err(ExchangeError::InvalidCaBundle)
}

fn validate_server_name<'b>(
    host: &str,
    scratch: &'b mut [u8; MAX_HOST_LEN + 1],
) -> Result<&'b CStr, ExchangeError> {
    if host.is_empty() || host.len() > MAX_HOST_LEN {
        return Err(ExchangeError::InvalidServerName);
    }
    if host.parse::<core::net::IpAddr>().is_ok() {
        return Err(ExchangeError::InvalidServerName);
    }
    if !host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
        || host.starts_with(['-', '.'])
        || host.ends_with('-')
    {
        return Err(ExchangeError::InvalidServerName);
    }
    scratch[..host.len()].copy_from_slice(host.as_bytes());
    scratch[host.len()] = 0;
    CStr::from_bytes_with_nul(&scratch[..=host.len()]).map_err(|_| ExchangeError::InvalidServerName)
}

/// Perform an authenticated HTTPS/1.1 request-response exchange over `stream`.
///
/// Requires `clock_valid` to be true, verifies the server's certificate chain,
/// SNI/SAN hostname and validity dates against `ca_chain`, writes `request`,
/// and returns a validated [`Response`].
pub async fn https_exchange<'r, S>(
    tls: TlsReference<'_>,
    ca_chain: &Certificate<'_>,
    clock_valid: bool,
    stream: S,
    host: &str,
    request: &[u8],
    response: &'r mut [u8],
) -> Result<Response<'r>, ExchangeError>
where
    S: Read + Write,
{
    if !clock_valid {
        return Err(ExchangeError::ClockNotSynchronized);
    }
    let mut host_buf = [0u8; MAX_HOST_LEN + 1];
    let server_name = validate_server_name(host, &mut host_buf)?;

    let mut client_config = ClientSessionConfig::new();
    client_config.ca_chain = Some(ca_chain.clone());
    client_config.server_name = Some(server_name);
    client_config.auth_mode = AuthMode::Required;
    client_config.min_version = TlsVersion::Tls1_2;
    client_config.alpn_protocols = Some(ALPN_HTTP11);
    client_config.skip_hostname_verification = false;

    let session_config = SessionConfig::Client(client_config);
    let mut session = Session::new(tls, stream, &session_config).map_err(ExchangeError::Tls)?;
    session.connect().await.map_err(ExchangeError::Tls)?;
    if session.tls_verification_details() != 0 {
        return Err(ExchangeError::CertificateVerificationFailed);
    }

    session
        .write_all(request)
        .await
        .map_err(ExchangeError::Tls)?;
    session.flush().await.map_err(ExchangeError::Tls)?;

    let parsed = http::read_and_parse_response(&mut session, response)
        .await
        .map_err(ExchangeError::Response)?;
    let _ = session.close().await;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use core::{
        convert::Infallible,
        future::poll_fn,
        sync::atomic::{AtomicI64, Ordering},
        task::{Poll, Waker},
    };
    use embassy_futures::{block_on, join::join};
    use embedded_io_async::ErrorType;
    use mbedtls_rs::{Credentials as TlsCredentials, PrivateKey, ServerSessionConfig, Tls};
    use rand_core::{TryCryptoRng, TryRng};
    use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Mutex as StdMutex};

    const TEST_CA_PEM: &CStr = c"-----BEGIN CERTIFICATE-----\n\
MIIBcTCCARigAwIBAgIUf9Zsk0Ucil6O7Ypxm7EYYpmj3NowCgYIKoZIzj0EAwIw\n\
FzEVMBMGA1UEAwwMVGVzdCBSb290IENBMB4XDTI1MDEwMTAwMDAwMFoXDTM1MDEw\n\
MTAwMDAwMFowFzEVMBMGA1UEAwwMVGVzdCBSb290IENBMFkwEwYHKoZIzj0CAQYI\n\
KoZIzj0DAQcDQgAEjWkpdCUcCRrzeR5o8FZ6OHaPYJI5Bcg34YkHw4Ihuy+5Innc\n\
afWaIXk8hm22hrIiOgZheY7AdhVUmIunA/QXDqNCMEAwDwYDVR0TAQH/BAUwAwEB\n\
/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFL7h2Judz33B61lQlHnqZMMBDEpz\n\
MAoGCCqGSM49BAMCA0cAMEQCIBOOyT6iEByokIhUrJeMF9RXCERqmlaVrYE8y+fm\n\
5eoKAiBkdoX4IY1HpK2CPRyfRwPRokxnOZZ6CPxCne+35RNDSw==\n\
-----END CERTIFICATE-----\n";

    const UNTRUSTED_CA_PEM: &CStr = c"-----BEGIN CERTIFICATE-----\n\
MIIBezCCASKgAwIBAgIUX3zEyQkoGRKau399SOI9MYCneZ4wCgYIKoZIzj0EAwIw\n\
HDEaMBgGA1UEAwwRVW50cnVzdGVkIFJvb3QgQ0EwHhcNMjUwMTAxMDAwMDAwWhcN\n\
MzUwMTAxMDAwMDAwWjAcMRowGAYDVQQDDBFVbnRydXN0ZWQgUm9vdCBDQTBZMBMG\n\
ByqGSM49AgEGCCqGSM49AwEHA0IABPT5Q6gR76dpU+A+nUumP6TXw5fGmXhsYhOz\n\
DO75VG6mWpw6XCaX7KB2a+D8yf2RQ70NcmlRWlon+Qwh4gGogrWjQjBAMA8GA1Ud\n\
EwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBQbTaZsTcuEeE2V\n\
aqLxV85fWSp8gDAKBggqhkjOPQQDAgNHADBEAiAdWHFroiZ4uReyns7Rq4K9qnvR\n\
O/bD/GcVTyq52sBgLAIgc8YTHFmQFUVgMt8l0QnfWXQOKxlfXxDQW5bCtds2CEM=\n\
-----END CERTIFICATE-----\n";

    // Signed by TEST_CA_PEM; Subject CN=cn-only.example.test;
    // SAN=DNS:api.porssisahko.net, DNS:project.supabase.co;
    // Valid 2026-01-01T00:00:00Z .. 2027-01-01T00:00:00Z.
    const SERVER_CERT_PEM: &CStr = c"-----BEGIN CERTIFICATE-----\n\
MIIBoTCCAUigAwIBAgIUaJZF4NP8KGWMykvs5TAd1rwTm7owCgYIKoZIzj0EAwIw\n\
FzEVMBMGA1UEAwwMVGVzdCBSb290IENBMB4XDTI2MDEwMTAwMDAwMFoXDTI3MDEw\n\
MTAwMDAwMFowHzEdMBsGA1UEAwwUY24tb25seS5leGFtcGxlLnRlc3QwWTATBgcq\n\
hkjOPQIBBggqhkjOPQMBBwNCAARSKFV5CfPNUPWVyOdI9eQ9/sFAlz4aCNtFyJui\n\
M2XxHVqTjCMaecbYsK2y9jHd0FNr4YFq9EAIxt/vWT5KfNYPo2owaDAMBgNVHRMB\n\
Af8EAjAAMA4GA1UdDwEB/wQEAwIHgDATBgNVHSUEDDAKBggrBgEFBQcDATAzBgNV\n\
HREELDAqghNhcGkucG9yc3Npc2Foa28ubmV0ghNwcm9qZWN0LnN1cGFiYXNlLmNv\n\
MAoGCCqGSM49BAMCA0cAMEQCIGl0/HkRSUa5oiZLFcb6VHuB1hckYDOVC3uEt5XZ\n\
lT8JAiB7WsRreszMJlRyPAA8qcpYPNpsq5+Y196J6wn0kOTYUg==\n\
-----END CERTIFICATE-----\n";

    const SERVER_KEY_PEM: &CStr = c"-----BEGIN EC PRIVATE KEY-----\n\
MHcCAQEEIHo26/fVieI7uH/J54GZ+DR91H5s/7Q267MprTyfJyNtoAoGCCqGSM49\n\
AwEHoUQDQgAEUihVeQnzzVD1lcjnSPXkPf7BQJc+GgjbRcibojNl8R1ak4wjGnnG\n\
2LCtsvYx3dBTa+GBavRACMbf71k+SnzWDw==\n\
-----END EC PRIVATE KEY-----\n";

    static TLS_LOCK: StdMutex<()> = StdMutex::new(());
    static TEST_NOW_MS: AtomicI64 = AtomicI64::new(0);
    static TEST_TIMER: TestTimer = TestTimer;
    static TEST_CLOCK: WallClock = WallClock::new();
    static TEST_MBEDTLS_CLOCK: MbedtlsClock<'static> = MbedtlsClock::new(&TEST_CLOCK, &TEST_TIMER);

    struct TestTimer;

    impl MbedtlsTimer for TestTimer {
        fn now(&self) -> i64 {
            TEST_NOW_MS.load(Ordering::Relaxed)
        }
    }

    struct TestRng(u64);

    impl TestRng {
        fn next_word(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
    }

    impl TryRng for TestRng {
        type Error = Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(self.next_word() as u32)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(self.next_word())
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            for chunk in dst.chunks_mut(8) {
                let bytes = self.next_word().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
            Ok(())
        }
    }

    impl TryCryptoRng for TestRng {}

    #[derive(Default)]
    struct PipeState {
        c2s: VecDeque<u8>,
        s2c: VecDeque<u8>,
        c2s_closed: bool,
        s2c_closed: bool,
        client_waker: Option<Waker>,
        server_waker: Option<Waker>,
    }

    struct DuplexEnd {
        state: Rc<RefCell<PipeState>>,
        is_client: bool,
    }

    impl Drop for DuplexEnd {
        fn drop(&mut self) {
            let mut state = self.state.borrow_mut();
            let waker = if self.is_client {
                state.c2s_closed = true;
                state.server_waker.take()
            } else {
                state.s2c_closed = true;
                state.client_waker.take()
            };
            drop(state);
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    impl ErrorType for DuplexEnd {
        type Error = embedded_io_async::ErrorKind;
    }

    impl Read for DuplexEnd {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            if buf.is_empty() {
                return Ok(0);
            }
            poll_fn(|cx| {
                let mut state = self.state.borrow_mut();
                let peer_closed = if self.is_client {
                    state.s2c_closed
                } else {
                    state.c2s_closed
                };
                let incoming = if self.is_client {
                    &mut state.s2c
                } else {
                    &mut state.c2s
                };
                if !incoming.is_empty() {
                    let count = buf.len().min(incoming.len());
                    for slot in &mut buf[..count] {
                        *slot = incoming.pop_front().unwrap();
                    }
                    Poll::Ready(Ok(count))
                } else if peer_closed {
                    Poll::Ready(Ok(0))
                } else {
                    if self.is_client {
                        state.client_waker = Some(cx.waker().clone());
                    } else {
                        state.server_waker = Some(cx.waker().clone());
                    }
                    Poll::Pending
                }
            })
            .await
        }
    }

    impl Write for DuplexEnd {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            let mut state = self.state.borrow_mut();
            let peer_closed = if self.is_client {
                state.s2c_closed
            } else {
                state.c2s_closed
            };
            if peer_closed {
                return Err(embedded_io_async::ErrorKind::BrokenPipe);
            }
            let waker = if self.is_client {
                state.c2s.extend(buf);
                state.server_waker.take()
            } else {
                state.s2c.extend(buf);
                state.client_waker.take()
            };
            drop(state);
            if let Some(waker) = waker {
                waker.wake();
            }
            Ok(buf.len())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn duplex_pair() -> (DuplexEnd, DuplexEnd) {
        let state = Rc::new(RefCell::new(PipeState::default()));
        (
            DuplexEnd {
                state: Rc::clone(&state),
                is_client: true,
            },
            DuplexEnd {
                state,
                is_client: false,
            },
        )
    }

    fn run_fixture_exchange<'r>(
        ca_pem: &CStr,
        wall_time: Option<DateTime<Utc>>,
        clock_valid: bool,
        host: &str,
        request: &[u8],
        server_reply: &[u8],
        response_buf: &'r mut [u8],
    ) -> Result<Response<'r>, ExchangeError> {
        TEST_NOW_MS.store(1_000, Ordering::Relaxed);
        match wall_time {
            Some(dt) => {
                assert!(TEST_CLOCK.set_unix_secs(dt.timestamp() as u64, 1_000));
            }
            None => TEST_CLOCK.clear(),
        }
        unsafe {
            install_hooks(&TEST_TIMER, &TEST_MBEDTLS_CLOCK);
        }
        let mut rng = TestRng(0x1234_5678_9ABC_DEF0);
        // SAFETY: `tls` is dropped at the end of this function before `rng` goes out of scope.
        let tls = unsafe { Tls::new_local_borrows(&mut rng) }.unwrap();
        let ca_chain = parse_ca_pem(ca_pem)?;
        let server_cert = Certificate::new(X509::PEM(SERVER_CERT_PEM)).unwrap();
        let server_key = PrivateKey::new(X509::PEM(SERVER_KEY_PEM), None).unwrap();

        let (client_io, server_io) = duplex_pair();
        let server_fut = async {
            let mut server_config = ServerSessionConfig::new(TlsCredentials {
                certificate: server_cert,
                private_key: server_key,
            });
            server_config.alpn_protocols = Some(ALPN_HTTP11);
            let mut server = Session::new(
                tls.reference(),
                server_io,
                &SessionConfig::Server(server_config),
            )
            .unwrap();
            if server.connect().await.is_ok() {
                let mut req_buf = [0u8; 512];
                let _ = server.read(&mut req_buf).await;
                let _ = server.write_all(server_reply).await;
                let _ = server.flush().await;
                let _ = server.close().await;
            }
        };
        let client_fut = https_exchange(
            tls.reference(),
            &ca_chain,
            clock_valid,
            client_io,
            host,
            request,
            response_buf,
        );
        let (client_result, ()) = block_on(join(client_fut, server_fut));
        client_result
    }

    #[test]
    fn wall_clock_enforces_minimum_epoch_and_monotonicity() {
        let clock = WallClock::new();
        assert_eq!(clock.now(0), None);
        assert!(!clock.set_unix_secs(clock::MIN_VALID_UNIX_SECS - 1, 100));
        assert_eq!(clock.now(100), None);

        let valid = 1_795_190_400; // 2026-11-20T16:00:00Z
        assert!(clock.set_unix_secs(valid, 1_000));
        assert_eq!(
            clock.now(6_500),
            DateTime::from_timestamp(valid as i64 + 5, 0)
        );
        // Rejects invalid update without clobbering existing anchor.
        assert!(!clock.set_unix_secs(42, 7_000));
        assert!(clock.is_valid(7_000));
        // Backwards monotonic reading fails closed.
        assert_eq!(clock.now(500), None);
        clock.clear();
        assert_eq!(clock.now(7_000), None);
    }

    #[test]
    fn loads_default_embedded_ca_bundle_and_validates_hostnames() {
        let _guard = TLS_LOCK.lock().unwrap();
        assert!(load_default_ca_bundle().is_ok());

        let mut buf = [0u8; MAX_HOST_LEN + 1];
        assert!(validate_server_name("api.porssisahko.net", &mut buf).is_ok());
        for invalid in [
            "",
            "192.168.1.1",
            "::1",
            "-leading.example.com",
            "trailing.example.com-",
            ".dot.example.com",
            "bad host.example.com",
            "api.porssisahko.net/path",
        ] {
            assert!(
                matches!(
                    validate_server_name(invalid, &mut buf),
                    Err(ExchangeError::InvalidServerName)
                ),
                "{invalid}"
            );
        }
        let too_long = "a".repeat(MAX_HOST_LEN + 1);
        assert!(matches!(
            validate_server_name(&too_long, &mut buf),
            Err(ExchangeError::InvalidServerName)
        ));
    }

    #[test]
    fn verified_tls_succeeds_for_content_length_and_chunked_responses() {
        let _guard = TLS_LOCK.lock().unwrap();
        let valid_time = Utc.with_ymd_and_hms(2026, 11, 20, 18, 0, 0).unwrap();

        let mut buf = [0u8; 512];
        let reply = run_fixture_exchange(
            TEST_CA_PEM,
            Some(valid_time),
            true,
            "api.porssisahko.net",
            b"GET /v2/price.json?date=2026-11-20T18:00:00.000Z HTTP/1.1\r\nHost: api.porssisahko.net\r\nConnection: close\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\n\r\n{\"price\":11.972}",
            &mut buf,
        )
        .unwrap();
        assert_eq!(reply.status, 200);
        assert_eq!(
            crate::electricity::parse_price_response(core::str::from_utf8(reply.body).unwrap())
                .map(|p| p.tenths_c_kwh()),
            Some(120)
        );

        let mut chunked_buf = [0u8; 512];
        let supa_reply = run_fixture_exchange(
            TEST_CA_PEM,
            Some(valid_time),
            true,
            "project.supabase.co",
            b"POST /rest/v1/training_records HTTP/1.1\r\nHost: project.supabase.co\r\nContent-Length: 2\r\n\r\n{}",
            b"HTTP/1.1 201 Created\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
            &mut chunked_buf,
        )
        .unwrap();
        assert_eq!(supa_reply.status, 201);
        assert!(supa_reply.body.is_empty());
    }

    #[test]
    fn rejects_wrong_host_cn_fallback_unknown_ca_invalid_dates_and_missing_clock() {
        let _guard = TLS_LOCK.lock().unwrap();
        let valid_time = Utc.with_ymd_and_hms(2026, 11, 20, 18, 0, 0).unwrap();
        let ok_http = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        let req = b"GET / HTTP/1.1\r\nHost: test\r\n\r\n";

        // Wrong host is rejected.
        let mut buf = [0u8; 256];
        assert!(matches!(
            run_fixture_exchange(
                TEST_CA_PEM,
                Some(valid_time),
                true,
                "wrong.example.test",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::Tls(_))
        ));

        // Certificate has CN=cn-only.example.test alongside DNS SANs; RFC 6125 forbids CN fallback.
        assert!(matches!(
            run_fixture_exchange(
                TEST_CA_PEM,
                Some(valid_time),
                true,
                "cn-only.example.test",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::Tls(_))
        ));

        // Untrusted CA is rejected.
        assert!(matches!(
            run_fixture_exchange(
                UNTRUSTED_CA_PEM,
                Some(valid_time),
                true,
                "api.porssisahko.net",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::Tls(_))
        ));

        // Expired certificate (after 2027-01-01) is rejected.
        let expired_time = Utc.with_ymd_and_hms(2027, 6, 1, 0, 0, 0).unwrap();
        assert!(matches!(
            run_fixture_exchange(
                TEST_CA_PEM,
                Some(expired_time),
                true,
                "api.porssisahko.net",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::Tls(_))
        ));

        // Not-yet-valid certificate (before 2026-01-01) is rejected.
        let early_time = Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap();
        assert!(matches!(
            run_fixture_exchange(
                TEST_CA_PEM,
                Some(early_time),
                true,
                "api.porssisahko.net",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::Tls(_))
        ));

        // Explicit pre-flight clock guard fails closed before touching transport.
        assert!(matches!(
            run_fixture_exchange(
                TEST_CA_PEM,
                None,
                false,
                "api.porssisahko.net",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::ClockNotSynchronized)
        ));

        // MbedTLS X.509 wall-clock hook itself fails closed when WallClock returns None.
        assert!(matches!(
            run_fixture_exchange(
                TEST_CA_PEM,
                None,
                true,
                "api.porssisahko.net",
                req,
                ok_http,
                &mut buf,
            ),
            Err(ExchangeError::Tls(_))
        ));
    }
}
