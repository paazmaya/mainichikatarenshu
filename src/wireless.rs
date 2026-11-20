use embassy_executor::Spawner;
use embassy_net::{
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    Config as NetConfig, IpAddress, Ipv4Address, Runner, Stack, StackResources,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use esp_hal::peripherals::WIFI;
use esp_radio::wifi::{
    sta::StationConfig, AuthenticationMethodConfig, Config, Interface, WifiController,
};
use mainichikatarenshu::electricity;
use mainichikatarenshu::network_config::Credentials;
use mainichikatarenshu::sntp;
use static_cell::StaticCell;

pub static CREDENTIALS: Channel<CriticalSectionRawMutex, Credentials, 1> = Channel::new();
pub static TIME_SYNC: Channel<CriticalSectionRawMutex, u64, 1> = Channel::new();
pub static PRICE_UPDATE: Channel<CriticalSectionRawMutex, heapless::String<16>, 1> = Channel::new();
pub static RECORD_SYNC_REQUEST: Channel<
    CriticalSectionRawMutex,
    mainichikatarenshu::training::Record,
    1,
> = Channel::new();
pub static RECORD_SYNCED: Channel<CriticalSectionRawMutex, chrono::NaiveDate, 1> = Channel::new();
static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

#[embassy_executor::task]
pub async fn wifi_task(wifi: WIFI<'static>, spawner: Spawner) {
    let credentials = CREDENTIALS.receive().await;
    let mut controller = match WifiController::new(wifi, Default::default()) {
        Ok(controller) => controller,
        Err(error) => {
            esp_println::println!("Wi-Fi initialization failed: {error:?}; restart to retry");
            return;
        }
    };
    let ssid = match credentials.ssid.as_str().try_into() {
        Ok(ssid) => ssid,
        Err(error) => {
            esp_println::println!("Invalid Wi-Fi SSID: {error:?}; restart to retry");
            return;
        }
    };
    let password = match credentials.password.as_str().try_into() {
        Ok(password) => password,
        Err(error) => {
            esp_println::println!("Invalid Wi-Fi password: {error:?}; restart to retry");
            return;
        }
    };
    let config = Config::Station(
        StationConfig::default()
            .with_ssid(ssid)
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(password)),
    );
    if let Err(error) = controller.set_config(&config) {
        esp_println::println!("Wi-Fi configuration failed: {error:?}; restart to retry");
        return;
    }

    let rng = esp_hal::rng::Rng::new();
    let seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());
    let (stack, runner) = embassy_net::new(
        Interface::station(),
        NetConfig::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        seed,
    );
    match network_task(runner) {
        Ok(task) => spawner.spawn(task),
        Err(error) => {
            esp_println::println!("Network task failed to start: {error:?}; restart to retry");
            return;
        }
    }
    loop {
        esp_println::println!("Connecting to Wi-Fi");
        match controller.connect_async().await {
            Ok(_) => {
                esp_println::println!("Wi-Fi associated; waiting for DHCP");
                let mut last_address = None;
                let mut time_synced = false;
                while controller.is_connected() {
                    let address = stack.config_v4().map(|config| config.address);
                    if address != last_address {
                        match address {
                            Some(address) => {
                                esp_println::println!("Wi-Fi IPv4: {address}");
                                if !time_synced {
                                    if let Some(unix_secs) = query_sntp(stack).await {
                                        let _ = TIME_SYNC.try_send(unix_secs);
                                        time_synced = true;
                                    }
                                }
                                // Fetch electricity price
                                if let Some(price_str) = fetch_electricity_price(stack).await {
                                    let _ = PRICE_UPDATE.try_send(price_str);
                                }
                            }
                            None => esp_println::println!("Wi-Fi IPv4 lease lost"),
                        }
                        last_address = address;
                    }
                    if let Ok(record) = RECORD_SYNC_REQUEST.try_receive() {
                        if sync_record_to_supabase(stack, &record).await {
                            let _ = RECORD_SYNCED.try_send(record.date);
                        }
                    }
                    Timer::after(Duration::from_secs(2)).await;
                }
                esp_println::println!("Wi-Fi disconnected");
            }
            Err(error) => esp_println::println!("Wi-Fi connection failed: {error:?}"),
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

async fn query_sntp(stack: Stack<'static>) -> Option<u64> {
    esp_println::println!("Querying SNTP time server...");
    // pool.ntp.org fallback IP (e.g. 216.239.35.0 time.google.com or cloudflare 162.159.200.1)
    // First try DNS resolution for pool.ntp.org, fallback to known public NTP IP (162.159.200.1 / time.cloudflare.com)
    let server_ip = match stack
        .dns_query("pool.ntp.org", embassy_net::dns::DnsQueryType::A)
        .await
    {
        Ok(addrs) => match addrs.into_iter().next() {
            Some(ip) => ip,
            None => IpAddress::Ipv4(Ipv4Address::new(162, 159, 200, 1)),
        },
        Err(e) => {
            esp_println::println!("DNS lookup failed ({e:?}), using fallback NTP IP");
            IpAddress::Ipv4(Ipv4Address::new(162, 159, 200, 1))
        }
    };

    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buf = [0u8; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buf = [0u8; 128];

    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);

    if let Err(e) = socket.bind(0) {
        esp_println::println!("Failed to bind UDP socket for SNTP: {e:?}");
        return None;
    }

    let request = sntp::create_request();
    if let Err(e) = socket.send_to(&request, (server_ip, sntp::NTP_PORT)).await {
        esp_println::println!("Failed to send SNTP request: {e:?}");
        return None;
    }

    let mut response_buf = [0u8; 64];
    // Wait up to 5 seconds for response
    let receive_future = socket.recv_from(&mut response_buf);
    match embassy_time::with_timeout(Duration::from_secs(5), receive_future).await {
        Ok(Ok((len, _from))) => match sntp::parse_response(&response_buf[..len]) {
            Ok(unix_secs) => {
                esp_println::println!("SNTP sync success! Unix timestamp: {unix_secs}");
                Some(unix_secs)
            }
            Err(err) => {
                esp_println::println!("Failed to parse SNTP response: {err:?}");
                None
            }
        },
        Ok(Err(e)) => {
            esp_println::println!("UDP receive error: {e:?}");
            None
        }
        Err(_) => {
            esp_println::println!("SNTP request timed out");
            None
        }
    }
}

async fn fetch_electricity_price(stack: Stack<'static>) -> Option<heapless::String<16>> {
    esp_println::println!("Fetching electricity spot price...");
    // Use public Nord Pool / porssisahko API host or HTTP fallback
    // e.g. api.spot-hinta.fi or api.porssisahko.net
    let host = "api.porssisahko.net";
    let server_ip = match stack
        .dns_query(host, embassy_net::dns::DnsQueryType::A)
        .await
    {
        Ok(addrs) => match addrs.into_iter().next() {
            Some(ip) => ip,
            None => {
                esp_println::println!("Could not resolve price API host");
                return None;
            }
        },
        Err(e) => {
            esp_println::println!("DNS lookup failed for price host: {e:?}");
            return None;
        }
    };

    let mut rx_buf = [0u8; 1024];
    let mut tx_buf = [0u8; 256];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
    socket.set_timeout(Some(Duration::from_secs(5)));

    if let Err(e) = socket.connect((server_ip, 80)).await {
        esp_println::println!("Failed to connect to price API: {e:?}");
        return None;
    }

    // Send HTTP GET request
    use embedded_io_async::Write;
    let request = "GET /v1/latest-prices.json HTTP/1.1\r\nHost: api.porssisahko.net\r\nConnection: close\r\n\r\n";
    if let Err(e) = socket.write_all(request.as_bytes()).await {
        esp_println::println!("Failed to write HTTP request: {e:?}");
        return None;
    }

    let mut response_buf = [0u8; 1024];
    let mut total_read = 0;
    use embedded_io_async::Read;
    loop {
        match socket.read(&mut response_buf[total_read..]).await {
            Ok(0) => break,
            Ok(n) => {
                total_read += n;
                if total_read >= response_buf.len() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if let Ok(response_str) = core::str::from_utf8(&response_buf[..total_read]) {
        if let Some(price_eur_mwh) = electricity::parse_first_price_eur_mwh(response_str) {
            let entry = electricity::PriceEntry::from_eur_mwh(0, price_eur_mwh);
            let formatted = entry.format_price();
            esp_println::println!("Current electricity price: {formatted}");
            return Some(formatted);
        }
    }

    esp_println::println!("No price parsed from API response");
    None
}

async fn sync_record_to_supabase(
    stack: Stack<'static>,
    record: &mainichikatarenshu::training::Record,
) -> bool {
    use mainichikatarenshu::supabase::{
        format_post_request, format_record_json, is_http_success, SupabaseConfig,
    };

    esp_println::println!(
        "Syncing training record to Supabase for date {}...",
        record.date
    );

    // Default Supabase project host / endpoint (can be configured or point to project instance)
    let host = "mainichi-kata.supabase.co";
    let server_ip = match stack
        .dns_query(host, embassy_net::dns::DnsQueryType::A)
        .await
    {
        Ok(addrs) => match addrs.into_iter().next() {
            Some(ip) => ip,
            None => {
                esp_println::println!("Could not resolve Supabase host");
                return false;
            }
        },
        Err(e) => {
            esp_println::println!("DNS lookup failed for Supabase: {e:?}");
            return false;
        }
    };

    let mut rx_buf = [0u8; 1024];
    let mut tx_buf = [0u8; 1024];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
    socket.set_timeout(Some(Duration::from_secs(5)));

    if let Err(e) = socket.connect((server_ip, 80)).await {
        esp_println::println!("Failed to connect to Supabase: {e:?}");
        return false;
    }

    let config = SupabaseConfig::new(host, "anon-public-key", "training_records");
    let payload = format_record_json(record);
    let request = format_post_request(&config, &payload);

    use embedded_io_async::Write;
    if let Err(e) = socket.write_all(request.as_bytes()).await {
        esp_println::println!("Failed to write Supabase POST request: {e:?}");
        return false;
    }

    let mut response_buf = [0u8; 512];
    let mut total_read = 0;
    use embedded_io_async::Read;
    loop {
        match socket.read(&mut response_buf[total_read..]).await {
            Ok(0) => break,
            Ok(n) => {
                total_read += n;
                if total_read >= response_buf.len() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if let Ok(response_str) = core::str::from_utf8(&response_buf[..total_read]) {
        if is_http_success(response_str) {
            esp_println::println!("Supabase sync succeeded for record {}", record.date);
            return true;
        }
    }

    esp_println::println!("Supabase sync request did not return success");
    false
}

#[embassy_executor::task]
async fn network_task(mut runner: Runner<'static, Interface>) {
    runner.run().await;
}
