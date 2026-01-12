use crate::config::BME280_EMPTY_SAMPLE_MSG;
use crate::models::WeatherData;
use log::{error, info, warn};

const SPLASH_SCREEN: &str = r#"
  ____                              ____      
 / ___| _ __ ___   ___   __ _      |  _ \ ___ 
 \___ \| '_ ` _ \ / _ \ / _` |_____| |_) / __|
  ___) | | | | | | (_) | (_| |_____|  _ <\__ \
 |____/|_| |_| |_|\___/ \__, |     |_| \_\___/
                        |___/                         "#;

pub(crate) enum LogLevel {
    Info,
    Warn,
    Error,
}

pub(crate) fn print_splash_screen() {
    info!("{}", SPLASH_SCREEN);
}

pub(crate) fn log_message(level: LogLevel, message: &str, custom_ts: Option<&str>) {
    let uptime = crate::time_utils::get_uptime_string();
    let ts = custom_ts
        .map(|s| s.to_string())
        .unwrap_or_else(crate::time_utils::get_timestamp);
    let prefix = format!("{} [{}]", uptime, ts);

    match level {
        LogLevel::Error => error!("\x1b[31m{} {}\x1b[0m", prefix, message),
        LogLevel::Warn => warn!("\x1b[38;5;11m{} {}\x1b[0m", prefix, message),
        LogLevel::Info => info!("\x1b[38;5;40m{} {}\x1b[0m", prefix, message),
    }
}

pub(crate) fn log_weather_data(data: &WeatherData) {
    let env_msg = format!(
        "[ 🌡️ Temp {:.2}C | 💧Humidity {:.2}% | ☁️ Pressure {:.2} hPa ]",
        data.temperature, data.humidity, data.pressure
    );
    log_message(LogLevel::Info, &env_msg, Some(&data.timestamp));

    if let Some(voc) = data.voc {
        let voc_msg = format!("🍃 Indoor air quality (VOC) index: {}", voc);
        log_message(LogLevel::Info, &voc_msg, Some(&data.timestamp));
    }
}

pub(crate) fn log_sensor_error(sensor_name: &str, error: impl std::fmt::Debug) {
    log_message(
        LogLevel::Error,
        &format!("🚫 {} Error: {:?}", sensor_name, error),
        None,
    );
}

pub(crate) fn log_empty_sample() {
    log_message(LogLevel::Warn, BME280_EMPTY_SAMPLE_MSG, None);
}
