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

static REBOOT_SIGNAL: Signal<CriticalSectionRawMutex, RebootReason> = Signal::new();

/// Sensor polling task.
///
/// Continuously reads weather data from the sensor station at a fixed interval and manages data flow.
///
/// # Behavior
///
/// This task performs the following operations in an infinite loop:
/// 1. Reads sensor data from the `WeatherStation` (BME280 + SGP40)
/// 2. Logs the retrieved weather data to the console
/// 3. Checks if the SGP40 VOC sensor is stuck at `VOC=1` (a known failure mode)
/// 4. If a stuck condition is detected, signals the reboot supervisor to restart the MCU
/// 5. Attempts to send data to the network task via `NETWORK_CHANNEL` if the sending interval has elapsed
/// 6. Waits for `EXECUTION_DELAY_MS` before the next iteration
///
/// # Data Flow
///
/// - Successfully read sensor data is sent to `NETWORK_CHANNEL` for HTTP transmission
/// - The channel uses a non-blocking `try_send()` to avoid blocking if the network task is busy
/// - Data is only sent if `HTTP_SEND_INTERVAL_MS` has elapsed since the last sending
///
/// # SGP40 Stuck Detection
///
/// The SGP40 sensor can occasionally get stuck, returning `VOC=1`. When detected:
/// - A warning is logged
/// - The `REBOOT_SIGNAL` is triggered with `RebootReason::Sgp40StuckAtOne`
/// - The `reboot_supervisor_task` will handle the actual MCU restart
///
/// # Arguments
///
/// * `station` - A static mutable reference to the initialized `WeatherStation` instance
///
/// # Panics
///
/// This task runs indefinitely and should never panic under normal operation.
/// If sensor reads fail, they are logged and the task continues.
#[embassy_executor::task]
pub(crate) async fn sensor_task(station: &'static mut WeatherStation) {
    let mut last_send_time = Instant::now();
    let send_interval = Duration::from_millis(HTTP_SEND_INTERVAL_MS);

    loop {
        if let Some(data) = station.read_sensor_data().await {
            log_weather_data(&data);

            let is_stuck_at_one = station.sgp40_stuck_at_one(data.voc);

            if is_stuck_at_one {
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

/// Reboot supervisor.
///
/// Why this task exists:
/// - The Sensirion SGP40 may occasionally get "stuck" and keep returning `VOC=1` indefinitely.
/// - Instead of rebooting from inside the sensor loop (which tends to spread "reset logic"
///   across the codebase), the sensor task emits a single reboot request signal.
/// - This task is the only component allowed to perform a full MCU restart (`esp_restart()`),
///   keeping reset behavior centralized, testable (as policy), and easy to adjust.
///
/// Flow:
/// 1) `sensor_task` detects "SGP40 stuck at 1" **after a warm-up window**
/// 2) it signals `REBOOT_SIGNAL` with a `RebootReason`
/// 3) this task waits for the signal, optionally delays for a log flush, and reboots the MCU
#[embassy_executor::task]
pub(crate) async fn reboot_supervisor_task() {
    let reason = REBOOT_SIGNAL.wait().await;
    warn!("🔁 Reboot supervisor: reboot requested: {:?}", reason);

    Timer::after(Duration::from_millis(200)).await;

    unsafe { esp_idf_svc::sys::esp_restart() }
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
