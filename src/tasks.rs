use crate::config::{
    EXECUTION_DELAY_MS, HTTP_CONSUMER_ENDPOINT_URL, HTTP_SEND_INTERVAL_MS, HTTP_SENDING_ENABLED,
    is_sending_enabled,
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
    if !is_sending_enabled() {
        info!("📡 Network Task: Disabled via config. Standing by.");
        return;
    }

    // The "Phoenix" (reset on failure) approach:
    // To prevent a panic, 'connection is not in initial phase', - it is a known quirk of the esp-idf-svc HTTP client:
    // When a request fails (e.g., due to a socket timeout or the Router jitter),
    // the internal state of the EspHttpConnection remains "dirty."
    // When we try to call post() again on that same dirty connection,
    // the library panics because it's not in the "Initial" state it expects.
    // If a request fails, - we destroy the client and create a fresh one.
    // This resets the internal state machine and clears any "poisoned" sockets.
    // When we continue the worker loop, the client variable goes out of scope.
    // Its Drop implementation is called, which internally tells the ESP-IDF to close the socket and free the memory.
    'worker: loop {
        let mut client = match HttpClient::new() {
            Ok(c) => c,
            Err(e) => {
                error!("‼️ Network Task: Could not init HTTP client: {:?}", e);
                continue 'worker;
            }
        };

        info!("📡 Network Task: Ready and reusing connection.");

        loop {
            let data = NETWORK_CHANNEL.receive().await;

            match client.post_data(HTTP_CONSUMER_ENDPOINT_URL, &data) {
                Ok(status) if status == 200 || status == 201 => {
                    info!("📡 Network: Data posted (Status {})", status);
                }
                Ok(429) => {
                    warn!("📡 Network: Rate limited (429). Cooling down...");
                    Timer::after(Duration::from_secs(5)).await;
                }
                Ok(status) => error!("📡 Network: Server error (Status {})", status),
                Err(error) => {
                    error!(
                        "📡‼️ Network: Request failed: {:?}. Resetting http client...",
                        error
                    );
                    Timer::after(Duration::from_secs(2)).await;
                    continue 'worker;
                }
            }
        }
    }
}
