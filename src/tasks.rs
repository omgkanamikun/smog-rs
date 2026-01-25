use crate::config::{
    EXECUTION_DELAY_MS, HTTP_CONSUMER_ENDPOINT_URL, HTTP_SEND_INTERVAL_MS, is_sending_enabled,
};
use crate::logging::log_weather_data;
use crate::models::WeatherData;
use crate::network::HttpClient;
use crate::sensors::WeatherStation;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use log::{error, info, warn};

static NETWORK_CHANNEL: Channel<CriticalSectionRawMutex, WeatherData, 2> = Channel::new();

#[derive(Copy, Clone, Debug)]
enum RebootReason {
    Sgp40StuckAtOne,
}

#[derive(Copy, Clone, Debug)]
enum RebootReason {
    Sgp40StuckAtOne,
}

static REBOOT_SIGNAL: Signal<CriticalSectionRawMutex, RebootReason> = Signal::new();

const SGP_40_WARMUP_SECS: u64 = 60;
const SGP_40_STUCK_AT_ONE_THRESHOLD: u16 = 20;

struct Sgp40Health {
    boot_time: Instant,
    consecutive_one: u16,
}

impl Sgp40Health {
    fn new() -> Self {
        Self {
            boot_time: Instant::now(),
            consecutive_one: 0,
        }
    }

    fn observe(&mut self, voc: Option<u16>) -> bool {
        if self.boot_time.elapsed() < Duration::from_secs(SGP_40_WARMUP_SECS) {
            self.consecutive_one = 0;
            return false;
        }

        match voc {
            Some(1) => {
                self.consecutive_one = self.consecutive_one.saturating_add(1);
                self.consecutive_one >= SGP_40_STUCK_AT_ONE_THRESHOLD
            }
            Some(_) | None => {
                self.consecutive_one = 0;
                false
            }
        }
    }
}

#[embassy_executor::task]
pub(crate) async fn sensor_task(station: &'static mut WeatherStation) {
    let mut last_send_time = Instant::now();
    let send_interval = Duration::from_millis(HTTP_SEND_INTERVAL_MS);

    let mut sgp40health = Sgp40Health::new();

    loop {
        if let Some(data) = station.read_sensor_data().await {
            log_weather_data(&data);

            let is_stuck_with_one = sgp40health.observe(data.voc);
            if is_stuck_with_one {
                warn!("‼️ SGP40 appears stuck at VOC=1. Requesting reboot...");
                REBOOT_SIGNAL.signal(RebootReason::Sgp40StuckAtOne)
            }

            if last_send_time.elapsed() >= send_interval && NETWORK_CHANNEL.try_send(data).is_ok() {
                last_send_time = Instant::now();
            }
        }
        Timer::after(Duration::from_millis(EXECUTION_DELAY_MS)).await;
    }
}

#[embassy_executor::task]
pub(crate) async fn reboot_supervisor_task() {
    let reason = REBOOT_SIGNAL.wait().await;
    warn!("🔁 Reboot supervisor: reboot requested: {:?}", reason);

    Timer::after(Duration::from_millis(200)).await;

    unsafe {
        esp_idf_svc::sys::esp_restart();
    }

    unreachable!("Can't be reached after ESP32 Reboot");
}

/// The Http Client resets on every HTTP call to prevent ESP_FAIL 'connection is not in the initial phase'
/// It is a known quirk of the esp-idf-svc HTTP client.
/// This resets the internal state machine and clears any "poisoned" sockets.
///When we continue the worker loop, the client variable goes out of the scope.
/// Its Drop implementation is called, which internally tells the ESP-IDF to close the socket and free the memory.
#[embassy_executor::task]
pub(crate) async fn network_task() {
    if !is_sending_enabled() {
        info!("📡 Network Task: Disabled via config. Standing by.");
        return;
    }

    info!("📡 Network Task: Ready and reusing connection.");

    loop {
        let mut client = match HttpClient::new() {
            Ok(c) => c,
            Err(e) => {
                warn!("‼️ Network Task: Could not init HTTP client: {:?}", e);
                Timer::after(Duration::from_secs(2)).await;
                continue;
            }
        };

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
                continue;
            }
        }
    }
}
