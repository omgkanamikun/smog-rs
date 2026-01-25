use crate::config::{WIFI_PASS, WIFI_SSID};
use crate::models::WeatherData;
use anyhow::Result;
use embassy_time::Timer;
pub use embedded_svc::http::Status;
use embedded_svc::http::client::Client as HttpClientImpl;
use embedded_svc::io::Write;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration as WifiConfig, EspWifi};
use log::{info, warn};

pub(crate) async fn setup_wifi(
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> Result<EspWifi<'static>> {
    let mut wifi = EspWifi::new(modem, sys_loop, Some(nvs))?;

    wifi.set_configuration(&WifiConfig::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().expect("SSID is too long"),
        password: WIFI_PASS.try_into().expect("Password is too long"),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    wifi.start()?;

    info!("📶 WiFi starting...");

    Timer::after_millis(500).await;

    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 40;
    const MAX_CONNECTED_WAIT_TICKS: u32 = 40;

    loop {
        attempts += 1;

        info!("📶 WiFi connecting (attempt {})...", attempts);

        match wifi.connect() {
            Ok(_) => {
                let mut wait_counter = 0;

                while !wifi.is_connected()? {
                    Timer::after_millis(250).await;

                    wait_counter += 1;
                    if wait_counter > MAX_CONNECTED_WAIT_TICKS {
                        break;
                    }
                }

                if wifi.is_connected()? {
                    break;
                }
            }
            Err(e) => warn!("📶 Connect call failed: {:?}", e),
        }

        if attempts >= MAX_ATTEMPTS {
            anyhow::bail!("‼️📶 Failed to connect after {} attempts", attempts);
        }

        info!("📶 Connection refused or timed out, retrying in 2s...");

        Timer::after_millis(2000).await;
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;
    info!("📶 WiFi Connected! IP: {}", ip_info.ip);

    Ok(wifi)
}

pub(crate) struct HttpClient {
    client: HttpClientImpl<EspHttpConnection>,
}

impl HttpClient {
    pub(crate) fn new() -> Result<Self> {
        let config = Configuration {
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        };

        let connection = EspHttpConnection::new(&config)?;

        let client = HttpClientImpl::wrap(connection);

        Ok(Self { client })
    }

    pub(crate) fn post_data(&mut self, url: &str, data: &WeatherData) -> Result<u16> {
        let payload = serde_json::to_vec(data)?;
        let len = payload.len().to_string();

        let headers = [
            ("Content-Type", "application/json"),
            ("Content-Length", &len),
        ];

        let mut request = self.client.post(url, &headers)?;

        request.write_all(&payload)?;

        let response = request.submit()?;

        let status = response.status();
        Ok(status)
    }
}
