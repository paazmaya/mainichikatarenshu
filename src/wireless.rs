use embassy_executor::Spawner;
use embassy_net::{Config as NetConfig, Runner, StackResources};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use esp_hal::peripherals::WIFI;
use esp_radio::wifi::{
    sta::StationConfig, AuthenticationMethodConfig, Config, Interface, WifiController,
};
use mainichikatarenshu::network_config::Credentials;
use static_cell::StaticCell;

pub static CREDENTIALS: Channel<CriticalSectionRawMutex, Credentials, 1> = Channel::new();
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
                while controller.is_connected() {
                    let address = stack.config_v4().map(|config| config.address);
                    if address != last_address {
                        match address {
                            Some(address) => esp_println::println!("Wi-Fi IPv4: {address}"),
                            None => esp_println::println!("Wi-Fi IPv4 lease lost"),
                        }
                        last_address = address;
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

#[embassy_executor::task]
async fn network_task(mut runner: Runner<'static, Interface>) {
    runner.run().await;
}
