//! Wi-Fi station and authenticated HTTPS clients (`wifi` feature only).
//!
//! [`wifi_task`] waits for [`CREDENTIALS`], joins the network and, while
//! connected, synchronizes the clock over SNTP (unless already set), fetches
//! the current 15-minute electricity price over HTTPS and uploads pending
//! training records to Supabase over HTTPS. It talks to the main loop through
//! the synchronization primitives below.

use chrono::{DateTime, Utc};
use core::fmt::Write as _;
use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsQueryType,
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    Config as NetConfig, IpAddress, Ipv4Address, Runner, Stack, StackResources,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};
use embassy_time::{with_timeout, Duration, Instant, Timer};
use esp_hal::{
    peripherals::{ADC1, RNG, WIFI},
    rng::{Trng, TrngSource},
};
use esp_radio::wifi::{
    sta::StationConfig, AuthenticationMethodConfig, Config, Interface, WifiController,
};
use heapless::String;
use mainichikatarenshu::{
    electricity, http,
    network_config::Credentials,
    sntp,
    supabase::{self, OutcomeDelivery, SupabaseConfig, SyncOutcome},
    tls,
    training::Record,
};
use mbedtls_rs::{
    sys::hook::backend::embassy::timer::EmbassyTimer, Certificate, Tls, TlsReference,
};
use static_cell::StaticCell;

type Mutex = CriticalSectionRawMutex;

/// Credentials to connect with; only the first message is used.
pub static CREDENTIALS: Channel<Mutex, Credentials, 1> = Channel::new();
/// Latest Unix seconds from a successful SNTP query.
pub static TIME_SYNC: Signal<Mutex, u64> = Signal::new();
/// The formatted electricity price of the current 15-minute interval.
pub static PRICE_UPDATE: Channel<Mutex, String<16>, 1> = Channel::new();
/// The oldest pending record, to upload to Supabase.
pub static RECORD_SYNC_REQUEST: Channel<Mutex, Record, 1> = Channel::new();
/// The outcome of the last [`RECORD_SYNC_REQUEST`].
pub static RECORD_SYNC_RESULT: Channel<Mutex, SyncOutcome, 1> = Channel::new();

static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static TRNG_SOURCE: StaticCell<TrngSource<'static>> = StaticCell::new();
static TRNG: StaticCell<Trng> = StaticCell::new();
static TIMER: EmbassyTimer = EmbassyTimer;
static WALL_CLOCK: tls::WallClock = tls::WallClock::new();
static MBEDTLS_CLOCK: tls::MbedtlsClock<'static> = tls::MbedtlsClock::new(&WALL_CLOCK, &TIMER);

/// Fallback NTP server (time.cloudflare.com) if DNS fails.
const FALLBACK_NTP_SERVER: IpAddress = IpAddress::Ipv4(Ipv4Address::new(162, 159, 200, 1));
const NTP_HOST: &str = "pool.ntp.org";
const NETWORK_TIMEOUT: Duration = Duration::from_secs(10);
const TLS_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECTED_POLL_INTERVAL: Duration = Duration::from_secs(2);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Wait before retrying a failed SNTP query, price request or record upload.
const RETRY_DELAY: Duration = Duration::from_secs(60);

/// Record validated wall-clock Unix seconds for TLS certificate checks and price queries.
pub fn set_wall_clock(unix_secs: u64) {
    let _ = WALL_CLOCK.set_unix_secs(unix_secs, Instant::now().as_millis());
}

fn wall_clock_now() -> Option<DateTime<Utc>> {
    WALL_CLOCK.now(Instant::now().as_millis())
}

/// Enable hardware TRNG, install TLS hooks and spawn [`wifi_task`].
pub fn start(
    wifi: WIFI<'static>,
    rng: RNG<'static>,
    adc1: ADC1<'static>,
    spawner: Spawner,
) -> bool {
    TRNG_SOURCE.init(TrngSource::new(rng, adc1));
    let trng = match Trng::try_new() {
        Ok(trng) => TRNG.init(trng),
        Err(error) => {
            esp_println::println!("Hardware TRNG initialization failed: {error:?}");
            return false;
        }
    };
    // SAFETY: `TIMER` and `MBEDTLS_CLOCK` are `'static` and installed once before any TLS/X.509 call.
    unsafe {
        tls::install_hooks(&TIMER, &MBEDTLS_CLOCK);
    }
    let tls = match Tls::new(trng) {
        Ok(tls) => tls,
        Err(error) => {
            esp_println::println!("TLS initialization failed: {error:?}");
            return false;
        }
    };
    match wifi_task(wifi, tls, spawner) {
        Ok(task) => {
            spawner.spawn(task);
            true
        }
        Err(error) => {
            esp_println::println!("Wi-Fi task failed to start: {error:?}");
            false
        }
    }
}

/// Wi-Fi credentials staged over the serial console.
pub struct Provisioning {
    available: bool,
    submitted: bool,
    ssid: String<32>,
    password: String<63>,
}

impl Provisioning {
    /// `available` tells whether [`wifi_task`] is running.
    pub fn new(available: bool) -> Self {
        Self {
            available,
            submitted: false,
            ssid: String::new(),
            password: String::new(),
        }
    }

    /// Whether credentials can still be submitted.
    pub fn is_available(&self) -> bool {
        self.available && !self.submitted
    }

    /// Stage the network name for [`Self::staged_credentials`].
    pub fn stage_ssid(&mut self, ssid: &str) {
        match String::try_from(ssid) {
            Ok(ssid) => {
                self.ssid = ssid;
                esp_println::println!("SSID received");
            }
            Err(_) => {
                self.ssid.clear();
                esp_println::println!("SSID too long");
            }
        }
    }

    /// Stage the passphrase for [`Self::staged_credentials`].
    pub fn stage_password(&mut self, password: &str) {
        match String::try_from(password) {
            Ok(password) => {
                self.password = password;
                esp_println::println!("Password received");
            }
            Err(_) => {
                self.password.clear();
                esp_println::println!("Password too long");
            }
        }
    }

    /// Validated staged credentials, or `None` (with the reason printed).
    pub fn staged_credentials(&self) -> Option<Credentials> {
        if !self.available {
            esp_println::println!("Wi-Fi task unavailable; restart to retry");
            return None;
        }
        if self.submitted {
            esp_println::println!("Restart to change Wi-Fi credentials");
            return None;
        }
        Credentials::new(&self.ssid, &self.password)
            .inspect_err(|error| esp_println::println!("Invalid Wi-Fi configuration: {error:?}"))
            .ok()
    }

    /// Hand credentials to [`wifi_task`]; it accepts only one set per boot.
    pub fn submit(&mut self, credentials: Credentials) {
        if !self.is_available() {
            return;
        }
        match CREDENTIALS.try_send(credentials) {
            Ok(()) => {
                self.submitted = true;
                self.password.clear();
                esp_println::println!("Wi-Fi connection requested");
            }
            Err(_) => esp_println::println!("Wi-Fi request queue full"),
        }
    }
}

/// Connect with the first received credentials and keep reconnecting.
#[embassy_executor::task]
pub async fn wifi_task(wifi: WIFI<'static>, tls: Tls<'static>, spawner: Spawner) {
    let ca_chain = match tls::load_default_ca_bundle() {
        Ok(chain) => chain,
        Err(error) => {
            esp_println::println!("Failed to load TLS CA bundle: {error:?}; restart to retry");
            return;
        }
    };
    let credentials = CREDENTIALS.receive().await;
    let Some(mut controller) = configure_station(wifi, &credentials) else {
        return;
    };
    let Some(stack) = start_network(spawner) else {
        return;
    };
    let mut session = Session::default();
    loop {
        esp_println::println!("Connecting to Wi-Fi");
        match controller.connect_async().await {
            Ok(_) => {
                esp_println::println!("Wi-Fi associated; waiting for DHCP");
                session
                    .run(&controller, stack, tls.reference(), &ca_chain)
                    .await;
                esp_println::println!("Wi-Fi disconnected");
            }
            Err(error) => esp_println::println!("Wi-Fi connection failed: {error:?}"),
        }
        Timer::after(RECONNECT_DELAY).await;
    }
}

/// Create a WPA2-Personal station controller; prints the reason on failure.
fn configure_station(
    wifi: WIFI<'static>,
    credentials: &Credentials,
) -> Option<WifiController<'static>> {
    let mut controller = WifiController::new(wifi, Default::default())
        .inspect_err(|error| {
            esp_println::println!("Wi-Fi initialization failed: {error:?}; restart to retry")
        })
        .ok()?;
    let ssid = credentials
        .ssid
        .as_str()
        .try_into()
        .inspect_err(|error| {
            esp_println::println!("Invalid Wi-Fi SSID: {error:?}; restart to retry")
        })
        .ok()?;
    let password = credentials
        .password
        .as_str()
        .try_into()
        .inspect_err(|error| {
            esp_println::println!("Invalid Wi-Fi password: {error:?}; restart to retry")
        })
        .ok()?;
    let config = Config::Station(
        StationConfig::default()
            .with_ssid(ssid)
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(password)),
    );
    controller
        .set_config(&config)
        .inspect_err(|error| {
            esp_println::println!("Wi-Fi configuration failed: {error:?}; restart to retry")
        })
        .ok()?;
    Some(controller)
}

/// Create the DHCP network stack and spawn its runner task.
fn start_network(spawner: Spawner) -> Option<Stack<'static>> {
    let rng = esp_hal::rng::Rng::new();
    let seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());
    let (stack, runner) = embassy_net::new(
        Interface::station(),
        NetConfig::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        seed,
    );
    match network_task(runner) {
        Ok(task) => {
            spawner.spawn(task);
            Some(stack)
        }
        Err(error) => {
            esp_println::println!("Network task failed to start: {error:?}; restart to retry");
            None
        }
    }
}

#[embassy_executor::task]
async fn network_task(mut runner: Runner<'static, Interface>) {
    runner.run().await;
}

/// Network state kept across reconnections.
#[derive(Default)]
struct Session {
    /// Whether SNTP has synchronized the clock during this boot.
    sntp_synced: bool,
    /// Earliest time to retry a failed SNTP query.
    clock_retry_at: Option<Instant>,
    /// Tracks price updates successfully delivered to the main loop.
    prices: electricity::PriceUpdates,
    /// Earliest time to retry a failed price request.
    price_retry_at: Option<Instant>,
    /// Retains any record sync outcome whose channel was temporarily full.
    sync_outcomes: OutcomeDelivery,
    /// Earliest time to retry a failed Supabase record upload.
    sync_retry_at: Option<Instant>,
}

/// Whether a retry scheduled for `retry_at` may run now.
fn is_due(retry_at: Option<Instant>) -> bool {
    retry_at.is_none_or(|at| Instant::now() >= at)
}

impl Session {
    /// Serve network work until the station disconnects.
    async fn run(
        &mut self,
        controller: &WifiController<'static>,
        stack: Stack<'static>,
        tls: TlsReference<'_>,
        ca_chain: &Certificate<'_>,
    ) {
        let mut last_address = None;
        while controller.is_connected() {
            let address = stack.config_v4().map(|config| config.address);
            if address != last_address {
                match address {
                    Some(address) => esp_println::println!("Wi-Fi IPv4: {address}"),
                    None => esp_println::println!("Wi-Fi IPv4 lease lost"),
                }
                last_address = address;
            }
            if address.is_some() {
                if !self.sntp_synced && wall_clock_now().is_none() && is_due(self.clock_retry_at) {
                    self.sync_clock(stack).await;
                }
                self.refresh_price(stack, tls, ca_chain).await;
                self.sync_pending_record(stack, tls, ca_chain).await;
            }
            Timer::after(CONNECTED_POLL_INTERVAL).await;
        }
    }

    async fn sync_clock(&mut self, stack: Stack<'static>) {
        match query_sntp(stack).await {
            Some(unix_secs) => {
                set_wall_clock(unix_secs);
                TIME_SYNC.signal(unix_secs);
                self.sntp_synced = true;
                self.clock_retry_at = None;
            }
            None => self.clock_retry_at = Some(Instant::now() + RETRY_DELAY),
        }
    }

    /// Fetch the price once per 15-minute UTC slot, once a valid clock is available.
    async fn refresh_price(
        &mut self,
        stack: Stack<'static>,
        tls: TlsReference<'_>,
        ca_chain: &Certificate<'_>,
    ) {
        let Some(now) = wall_clock_now() else {
            return;
        };
        if !self.prices.needs_price(now) || !is_due(self.price_retry_at) {
            return;
        }
        let path = electricity::price_request_path(now);
        match fetch_electricity_price(stack, tls, ca_chain, &path).await {
            Some(price) => {
                match self
                    .prices
                    .deliver(now, price, |text| PRICE_UPDATE.try_send(text))
                {
                    Ok(()) => self.price_retry_at = None,
                    Err(_) => {
                        esp_println::println!("Price update queue full; retrying");
                        self.price_retry_at = Some(Instant::now() + RETRY_DELAY);
                    }
                }
            }
            None => self.price_retry_at = Some(Instant::now() + RETRY_DELAY),
        }
    }

    /// Flush any undelivered sync result, then upload the next requested record over HTTPS.
    async fn sync_pending_record(
        &mut self,
        stack: Stack<'static>,
        tls: TlsReference<'_>,
        ca_chain: &Certificate<'_>,
    ) {
        if self
            .sync_outcomes
            .flush(|outcome| RECORD_SYNC_RESULT.try_send(outcome))
            .is_err()
        {
            esp_println::println!("Record sync result queue full; retrying");
            return;
        }
        let Some(config) = supabase::CONFIGURED else {
            return;
        };
        if wall_clock_now().is_none() || !is_due(self.sync_retry_at) {
            return;
        }
        let Ok(record) = RECORD_SYNC_REQUEST.try_receive() else {
            return;
        };
        let outcome = if post_record(stack, tls, ca_chain, &config, &record).await {
            self.sync_retry_at = None;
            SyncOutcome::Synced(record.date)
        } else {
            self.sync_retry_at = Some(Instant::now() + RETRY_DELAY);
            SyncOutcome::Failed
        };
        self.sync_outcomes.stage(outcome);
        if self
            .sync_outcomes
            .flush(|result| RECORD_SYNC_RESULT.try_send(result))
            .is_err()
        {
            esp_println::println!("Record sync result queue full; retrying");
        }
    }
}

/// Resolve `host` to its first IPv4 address within [`NETWORK_TIMEOUT`].
async fn resolve_host(stack: Stack<'static>, host: &str) -> Option<IpAddress> {
    match with_timeout(NETWORK_TIMEOUT, stack.dns_query(host, DnsQueryType::A)).await {
        Ok(Ok(addresses)) => {
            let address = addresses.into_iter().next();
            if address.is_none() {
                esp_println::println!("No address found for {host}");
            }
            address
        }
        Ok(Err(error)) => {
            esp_println::println!("DNS lookup failed for {host}: {error:?}");
            None
        }
        Err(_) => {
            esp_println::println!("DNS lookup timed out for {host}");
            None
        }
    }
}

/// Send `request` to `host` over authenticated HTTPS on port 443 and validate the response.
async fn https_exchange<'r>(
    stack: Stack<'static>,
    tls: TlsReference<'_>,
    ca_chain: &Certificate<'_>,
    host: &str,
    request: &[u8],
    response: &'r mut [u8],
) -> Option<http::Response<'r>> {
    if wall_clock_now().is_none() {
        esp_println::println!("Refusing HTTPS to {host}: wall clock is not synchronized");
        return None;
    }
    let address = resolve_host(stack, host).await?;
    let mut rx_buffer = [0u8; 2048];
    let mut tx_buffer = [0u8; 2048];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(NETWORK_TIMEOUT));
    match with_timeout(NETWORK_TIMEOUT, socket.connect((address, tls::HTTPS_PORT))).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            esp_println::println!("Failed to connect to {host}: {error:?}");
            return None;
        }
        Err(_) => {
            esp_println::println!("TCP connection to {host} timed out");
            return None;
        }
    }
    match with_timeout(
        TLS_EXCHANGE_TIMEOUT,
        tls::https_exchange(
            tls,
            ca_chain,
            wall_clock_now().is_some(),
            &mut socket,
            host,
            request,
            response,
        ),
    )
    .await
    {
        Ok(Ok(reply)) => Some(reply),
        Ok(Err(error)) => {
            esp_println::println!("HTTPS exchange with {host} failed: {error:?}");
            None
        }
        Err(_) => {
            esp_println::println!("HTTPS exchange with {host} timed out");
            None
        }
    }
}

/// Query an SNTP server; returns Unix seconds.
async fn query_sntp(stack: Stack<'static>) -> Option<u64> {
    esp_println::println!("Querying SNTP time server...");
    let server = match resolve_host(stack, NTP_HOST).await {
        Some(address) => address,
        None => {
            esp_println::println!("Using fallback NTP server");
            FALLBACK_NTP_SERVER
        }
    };

    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buffer = [0u8; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0u8; 128];
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    if let Err(error) = socket.bind(0) {
        esp_println::println!("Failed to bind UDP socket for SNTP: {error:?}");
        return None;
    }
    if let Err(error) = socket
        .send_to(&sntp::create_request(), (server, sntp::NTP_PORT))
        .await
    {
        esp_println::println!("Failed to send SNTP request: {error:?}");
        return None;
    }

    let mut response = [0u8; 64];
    match embassy_time::with_timeout(NETWORK_TIMEOUT, socket.recv_from(&mut response)).await {
        Ok(Ok((length, _))) => match sntp::parse_response(&response[..length]) {
            Ok(unix_secs) => {
                esp_println::println!("SNTP sync success! Unix timestamp: {unix_secs}");
                Some(unix_secs)
            }
            Err(error) => {
                esp_println::println!("Failed to parse SNTP response: {error:?}");
                None
            }
        },
        Ok(Err(error)) => {
            esp_println::println!("UDP receive error: {error:?}");
            None
        }
        Err(_) => {
            esp_println::println!("SNTP request timed out");
            None
        }
    }
}

/// Fetch the price at `path` from the price API over HTTPS.
async fn fetch_electricity_price(
    stack: Stack<'static>,
    tls: TlsReference<'_>,
    ca_chain: &Certificate<'_>,
    path: &str,
) -> Option<electricity::Price> {
    esp_println::println!("Fetching electricity spot price...");
    let mut request: String<160> = String::new();
    // A 48-byte path plus fixed headers always fits.
    let _ = write!(
        request,
        "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        electricity::PRICE_API_HOST
    );
    let mut response = [0u8; 2048];
    let reply = https_exchange(
        stack,
        tls,
        ca_chain,
        electricity::PRICE_API_HOST,
        request.as_bytes(),
        &mut response,
    )
    .await?;
    if !reply.is_success() {
        esp_println::println!("Price API returned status {}", reply.status);
        return None;
    }
    let body = match core::str::from_utf8(reply.body) {
        Ok(body) => body,
        Err(error) => {
            esp_println::println!("Price API response is not UTF-8: {error}");
            return None;
        }
    };
    let price = electricity::parse_price_response(body);
    match price {
        Some(price) => esp_println::println!("Current electricity price: {}", price.format()),
        None => esp_println::println!("No price parsed from API response"),
    }
    price
}

/// Insert `record` into the Supabase table over HTTPS; returns whether it succeeded.
async fn post_record(
    stack: Stack<'static>,
    tls: TlsReference<'_>,
    ca_chain: &Certificate<'_>,
    config: &SupabaseConfig<'static>,
    record: &Record,
) -> bool {
    esp_println::println!(
        "Syncing training record to Supabase for date {}...",
        record.date
    );
    let request = match supabase::format_record_json(record)
        .and_then(|payload| supabase::format_post_request(config, &payload))
    {
        Ok(request) => request,
        Err(error) => {
            esp_println::println!("Supabase request does not fit: {error:?}");
            return false;
        }
    };
    let mut response = [0u8; 2048];
    let Some(reply) = https_exchange(
        stack,
        tls,
        ca_chain,
        config.host,
        request.as_bytes(),
        &mut response,
    )
    .await
    else {
        return false;
    };
    if reply.is_success() {
        esp_println::println!("Supabase sync succeeded for record {}", record.date);
        true
    } else {
        esp_println::println!("Supabase sync failed with status {}", reply.status);
        false
    }
}
