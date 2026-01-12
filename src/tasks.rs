use crate::config::EXECUTION_DELAY_MS;
use crate::logging::log_weather_data;
use crate::sensors::WeatherStation;
use embassy_time::{Duration, Timer};

#[embassy_executor::task]
pub(crate) async fn sensor_task(station: &'static mut WeatherStation) {
    loop {
        if let Some(data) = station.update().await {
            log_weather_data(&data);
        }
        Timer::after(Duration::from_millis(EXECUTION_DELAY_MS)).await;
    }
}
