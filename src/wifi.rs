use anyhow::{Context, Result};
use esp_idf_svc::wifi::{ClientConfiguration, Configuration};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::{EspNvsPartition, NvsDefault};
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::{info, warn};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use core::convert::From;

// Re-export AuthMethod for public use
pub use esp_idf_svc::wifi::AuthMethod;

#[derive(Debug)]
pub struct WifiNetwork<'a> {
    pub ssid: &'a str,
    pub password: &'a str,
    pub auth_method: AuthMethod,
}

impl<'a> WifiNetwork<'a> {
    pub const fn new(ssid: &'a str, password: &'a str) -> Self {
        Self {
            ssid,
            password,
            auth_method: AuthMethod::WPA2Personal,
        }
    }

    pub const fn with_auth(mut self, auth_method: AuthMethod) -> Self {
        self.auth_method = auth_method;
        self
    }
}

pub struct WifiManager<'a> {
    networks: &'a [WifiNetwork<'a>],
    wifi: Option<Box<BlockingWifi<EspWifi<'a>>>>,
    current_network: Option<&'a WifiNetwork<'a>>,
}

impl<'a> WifiManager<'a> {
    pub fn new(networks: &'a [WifiNetwork<'a>]) -> Self {
        Self {
            networks,
            wifi: None,
            current_network: None,
        }
    }

    pub fn connect(
        &mut self,
        modem: impl core::convert::Into<esp_idf_svc::hal::modem::Modem> + 'a,
    ) -> Result<()> {
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspNvsPartition::<NvsDefault>::take()?;

        let wifi = Box::new(BlockingWifi::wrap(
            EspWifi::new(modem.into(), sys_loop.clone(), Some(nvs))?,
            sys_loop,
        )?);

        self.wifi = Some(wifi);

        // First, scan for available networks
        let available_networks = self.scan_networks()?;
        info!("Found {} available networks", available_networks.len());

        // Try to connect to any of the known networks that are available
        for network in self.networks.iter() {
            if available_networks.contains(&network.ssid.to_string()) {
                info!("Attempting to connect to network: {}", network.ssid);

                if let Err(e) = self.connect_to_network(network) {
                    warn!("Failed to connect to {}: {}", network.ssid, e);
                    continue;
                }

                self.current_network = Some(network);
                info!("Successfully connected to {}", network.ssid);

                if let Some(ip_info) = self.get_ip_info()? {
                    info!("IP: {}", ip_info.ip);
                }

                return Ok(());
            }
        }

        Err(anyhow::anyhow!("No known networks available"))
    }

    fn scan_networks(&mut self) -> Result<Vec<String>> {
        let wifi = self.wifi.as_mut().context("WiFi not initialized")?;

        // Start WiFi in station mode for scanning
        wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
        wifi.start()?;

        // Scan for available networks
        let ap_infos = wifi.scan()?;
        let available_networks: Vec<String> = ap_infos.iter().map(|ap| ap.ssid.as_str().to_string()).collect();

        Ok(available_networks)
    }

    fn connect_to_network(&mut self, network: &'a WifiNetwork) -> Result<()> {
        let wifi = self.wifi.as_mut().context("WiFi not initialized")?;

        let wifi_config = Configuration::Client(ClientConfiguration {
            ssid: heapless::String::<32>::try_from(network.ssid).unwrap_or_default(),
            password: heapless::String::<64>::try_from(network.password).unwrap_or_default(),
            auth_method: network.auth_method,
            ..Default::default()
        });

        wifi.set_configuration(&wifi_config)?;
        wifi.start()?;
        wifi.connect()?;
        wifi.wait_netif_up()?;

        Ok(())
    }

    pub fn get_ip_info(&self) -> Result<Option<esp_idf_svc::ipv4::IpInfo>> {
        let wifi = self.wifi.as_ref().context("WiFi not initialized")?;
        match wifi.wifi().sta_netif().get_ip_info() {
            Ok(ip_info) => Ok(Some(ip_info)),
            Err(_) => Ok(None),
        }
    }

    pub fn get_current_network(&self) -> Option<&'a WifiNetwork> {
        self.current_network
    }

    pub fn is_connected(&self) -> bool {
        self.current_network.is_some()
    }
}

pub fn get_wifi_status(wifi: &mut EspWifi<'_>) -> String {
    if let Ok(ap_infos) = wifi.scan() {
        for ap in ap_infos {
            return format!("SSID: {}, Signal: {}%", ap.ssid, ap.signal_strength);
        }
    }
    "Not connected".to_string()
}
