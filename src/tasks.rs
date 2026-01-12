use crate::config::{
    EXECUTION_DELAY_MS, HTTP_CONSUMER_ENDPOINT_URL, HTTP_SEND_INTERVAL_MS, HTTP_SENDING_ENABLED,
};
use crate::logging::log_weather_data;
use crate::models::WeatherData;
use crate::network::HttpClient;
use crate::sensors::WeatherStation;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use log::{error, info, warn};

pub static NETWORK_CHANNEL: Channel<CriticalSectionRawMutex, WeatherData, 2> = Channel::new();

#[embassy_executor::task]
pub(crate) async fn sensor_task(station: &'static mut WeatherStation) {
    let mut last_send_time = embassy_time::Instant::now();
    let send_interval = Duration::from_millis(HTTP_SEND_INTERVAL_MS);

    loop {
        if let Some(data) = station.update().await {
            log_weather_data(&data);

            if last_send_time.elapsed() >= send_interval && NETWORK_CHANNEL.try_send(data).is_ok() {
                last_send_time = embassy_time::Instant::now();
            }
        }
        Timer::after(Duration::from_millis(EXECUTION_DELAY_MS)).await;
    }
}

#[embassy_executor::task]
pub(crate) async fn network_task() {
    if HTTP_SENDING_ENABLED != "true" {
        info!("📡 Network Task: Disabled via config. Standing by.");
        return;
    }

    let mut client = match HttpClient::new() {
        Ok(c) => c,
        Err(e) => {
            error!("‼️ Network Task: Could not init HTTP client: {:?}", e);
            return;
        }
    };

    info!("📡 Network Task: Ready and reusing connection.");

    let mut retry_delay = Duration::from_secs(5);

    loop {
        let data = NETWORK_CHANNEL.receive().await;

        match client.post_data(HTTP_CONSUMER_ENDPOINT_URL, &data) {
            Ok(status) if status == 200 || status == 201 => {
                info!("📡 Network: Data posted (Status {})", status);
                retry_delay = Duration::from_secs(5);
            }
            Ok(429) => {
                warn!("📡 Network: Rate limited (429). Cooling down...");
                Timer::after(retry_delay).await;
                retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
            }
            Ok(status) => error!("📡 Network: Server error (Status {})", status),
            Err(error) => {
                error!("📡‼️ Network: Critical failure: {:?}", error);
                Timer::after(Duration::from_secs(10)).await;
            }
        }
    }
}
