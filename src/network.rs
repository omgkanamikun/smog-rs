use crate::config::{WIFI_PASS, WIFI_SSID};
use anyhow::Result;
use embassy_time::{Duration, Timer};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
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

    Timer::after(Duration::from_millis(500)).await;

    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 40;
    loop {
        attempts += 1;
        info!("📶 WiFi connecting (attempt {})...", attempts);
        match wifi.connect() {
            Ok(_) => {
                let mut wait_counter = 0;

                while !wifi.is_connected()? {
                    Timer::after(Duration::from_millis(250)).await;

                    wait_counter += 1;
                    if wait_counter > MAX_ATTEMPTS {
                        break;
                    }
                }

                if wifi.is_connected()? {
                    break;
                }
            }
            Err(e) => warn!("📶 Connect call failed: {:?}", e),
        }

        if attempts >= 5 {
            anyhow::bail!("📶 Failed to connect after {} attempts", attempts);
        }

        info!("📶 Connection refused or timed out, retrying in 2s...");
        Timer::after(Duration::from_millis(2000)).await;
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;
    info!("📶 WiFi Connected! IP: {}", ip_info.ip);

    Ok(wifi)
}
