//! Wi-Fi station and network clients (`wifi` feature only).
//!
//! [`wifi_task`] waits for [`CREDENTIALS`], joins the network and, while
//! connected, synchronizes the clock over SNTP, fetches the current
//! electricity price every hour and uploads pending training records to
//! Supabase. It talks to the main loop only through the channels below.
//!
//! Both HTTP clients use plain HTTP on port 80. The price API and Supabase
//! only serve HTTPS, so those requests fail until TLS support is added.

use chrono::{DateTime, NaiveDate, Utc};
use core::fmt::Write as _;
use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsQueryType,
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    Config as NetConfig, IpAddress, Ipv4Address, Runner, Stack, StackResources,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Write;
use esp_hal::peripherals::WIFI;
use esp_radio::wifi::{
    sta::StationConfig, AuthenticationMethodConfig, Config, Interface, WifiController,
};
use heapless::String;
use mainichikatarenshu::{
    electricity, http,
    network_config::Credentials,
    sntp,
    supabase::{self, SupabaseConfig},
    training::Record,
};
use static_cell::StaticCell;

type Mutex = CriticalSectionRawMutex;

/// Credentials to connect with; only the first message is used.
pub static CREDENTIALS: Channel<Mutex, Credentials, 1> = Channel::new();
/// Unix seconds from a successful SNTP query.
pub static TIME_SYNC: Channel<Mutex, u64, 1> = Channel::new();
/// The formatted electricity price of the current hour.
pub static PRICE_UPDATE: Channel<Mutex, String<16>, 1> = Channel::new();
/// The oldest pending record, to upload to Supabase.
pub static RECORD_SYNC_REQUEST: Channel<Mutex, Record, 1> = Channel::new();
/// The outcome of the last [`RECORD_SYNC_REQUEST`].
pub static RECORD_SYNC_RESULT: Channel<Mutex, SyncOutcome, 1> = Channel::new();

static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

/// Fallback NTP server (time.cloudflare.com) if DNS fails.
const FALLBACK_NTP_SERVER: IpAddress = IpAddress::Ipv4(Ipv4Address::new(162, 159, 200, 1));
const NTP_HOST: &str = "pool.ntp.org";
const HTTP_PORT: u16 = 80;
const NETWORK_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECTED_POLL_INTERVAL: Duration = Duration::from_secs(2);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Wait before retrying a failed SNTP query or price request.
const RETRY_DELAY: Duration = Duration::from_secs(60);

/// Result of uploading one record.
#[derive(Clone, Copy, Debug)]
pub enum SyncOutcome {
    /// The record for this date was stored remotely and may be acknowledged.
    Synced(NaiveDate),
    /// The upload failed; the record stays pending.
    Failed,
}

/// Spawn [`wifi_task`]; returns whether the radio task is running.
pub fn start(wifi: WIFI<'static>, spawner: Spawner) -> bool {
    match wifi_task(wifi, spawner) {
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
pub async fn wifi_task(wifi: WIFI<'static>, spawner: Spawner) {
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
                session.run(&controller, stack).await;
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

/// Wall-clock time derived from an SNTP result and the monotonic timer.
#[derive(Clone, Copy)]
struct SyncedClock {
    unix_secs: u64,
    at: Instant,
}

impl SyncedClock {
    fn now(self) -> Option<DateTime<Utc>> {
        let secs = self.unix_secs + self.at.elapsed().as_secs();
        DateTime::from_timestamp(i64::try_from(secs).ok()?, 0)
    }
}

/// Network state kept across reconnections.
#[derive(Default)]
struct Session {
    clock: Option<SyncedClock>,
    /// Earliest time to retry a failed SNTP query.
    clock_retry_at: Option<Instant>,
    /// The last successfully fetched price path; a new path means a new hour.
    last_price_path: Option<String<48>>,
    /// Earliest time to retry a failed price request.
    price_retry_at: Option<Instant>,
}

/// Whether a retry scheduled for `retry_at` may run now.
fn is_due(retry_at: Option<Instant>) -> bool {
    retry_at.is_none_or(|at| Instant::now() >= at)
}

impl Session {
    /// Serve network work until the station disconnects.
    async fn run(&mut self, controller: &WifiController<'static>, stack: Stack<'static>) {
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
                if self.clock.is_none() && is_due(self.clock_retry_at) {
                    self.sync_clock(stack).await;
                }
                self.refresh_price(stack).await;
                sync_pending_record(stack).await;
            }
            Timer::after(CONNECTED_POLL_INTERVAL).await;
        }
    }

    async fn sync_clock(&mut self, stack: Stack<'static>) {
        match query_sntp(stack).await {
            Some(unix_secs) => {
                self.clock = Some(SyncedClock {
                    unix_secs,
                    at: Instant::now(),
                });
                let _ = TIME_SYNC.try_send(unix_secs);
            }
            None => self.clock_retry_at = Some(Instant::now() + RETRY_DELAY),
        }
    }

    /// Fetch the price once per Helsinki-local hour, once the time is known.
    async fn refresh_price(&mut self, stack: Stack<'static>) {
        let Some(now) = self.clock.and_then(SyncedClock::now) else {
            return;
        };
        let path = electricity::price_request_path(now);
        if self.last_price_path.as_ref() == Some(&path) || !is_due(self.price_retry_at) {
            return;
        }
        match fetch_electricity_price(stack, &path).await {
            Some(price) => {
                let _ = PRICE_UPDATE.try_send(price.format());
                self.last_price_path = Some(path);
                self.price_retry_at = None;
            }
            None => self.price_retry_at = Some(Instant::now() + RETRY_DELAY),
        }
    }
}

/// Resolve `host` to its first IPv4 address.
async fn resolve_host(stack: Stack<'static>, host: &str) -> Option<IpAddress> {
    match stack.dns_query(host, DnsQueryType::A).await {
        Ok(addresses) => {
            let address = addresses.into_iter().next();
            if address.is_none() {
                esp_println::println!("No address found for {host}");
            }
            address
        }
        Err(error) => {
            esp_println::println!("DNS lookup failed for {host}: {error:?}");
            None
        }
    }
}

/// Send `request` to `host` over plain HTTP and read the reply into `response`.
///
/// Returns the reply as text, truncated to the buffer size.
async fn http_exchange<'r>(
    stack: Stack<'static>,
    host: &str,
    request: &[u8],
    response: &'r mut [u8],
) -> Option<&'r str> {
    let address = resolve_host(stack, host).await?;
    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; 1024];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(NETWORK_TIMEOUT));
    if let Err(error) = socket.connect((address, HTTP_PORT)).await {
        esp_println::println!("Failed to connect to {host}: {error:?}");
        return None;
    }
    if let Err(error) = socket.write_all(request).await {
        esp_println::println!("Failed to send request to {host}: {error:?}");
        return None;
    }
    let mut length = 0;
    while length < response.len() {
        match socket.read(&mut response[length..]).await {
            Ok(0) | Err(_) => break,
            Ok(read) => length += read,
        }
    }
    let text = core::str::from_utf8(&response[..length]).ok();
    if text.is_none() {
        esp_println::println!("Response from {host} is not UTF-8");
    }
    text
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

/// Fetch the price at `path` from the price API.
async fn fetch_electricity_price(stack: Stack<'static>, path: &str) -> Option<electricity::Price> {
    esp_println::println!("Fetching electricity spot price...");
    let mut request: String<160> = String::new();
    // A 48-byte path plus fixed headers always fits.
    let _ = write!(
        request,
        "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        electricity::PRICE_API_HOST
    );
    let mut response = [0u8; 1024];
    let reply = http_exchange(
        stack,
        electricity::PRICE_API_HOST,
        request.as_bytes(),
        &mut response,
    )
    .await?;
    if !http::is_success(reply) {
        esp_println::println!("Price API returned status {:?}", http::status_code(reply));
        return None;
    }
    let price = electricity::parse_price_response(reply);
    match price {
        Some(price) => esp_println::println!("Current electricity price: {}", price.format()),
        None => esp_println::println!("No price parsed from API response"),
    }
    price
}

/// Upload the requested record, if any, and report the outcome.
async fn sync_pending_record(stack: Stack<'static>) {
    let Some(config) = supabase::CONFIGURED else {
        return;
    };
    let Ok(record) = RECORD_SYNC_REQUEST.try_receive() else {
        return;
    };
    let outcome = if post_record(stack, &config, &record).await {
        SyncOutcome::Synced(record.date)
    } else {
        SyncOutcome::Failed
    };
    let _ = RECORD_SYNC_RESULT.try_send(outcome);
}

/// Insert `record` into the Supabase table; returns whether it succeeded.
async fn post_record(
    stack: Stack<'static>,
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
    let mut response = [0u8; 512];
    let Some(reply) = http_exchange(stack, config.host, request.as_bytes(), &mut response).await
    else {
        return false;
    };
    if http::is_success(reply) {
        esp_println::println!("Supabase sync succeeded for record {}", record.date);
        true
    } else {
        esp_println::println!(
            "Supabase sync failed with status {:?}",
            http::status_code(reply)
        );
        false
    }
}
