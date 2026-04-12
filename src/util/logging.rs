use crate::domain::models::WeatherData;
use crate::util::time_utils::get_formatted_timestamp;
use log::{Level, error, info, warn};

const SPLASH_SCREEN: &str = r#"
  ____                              ____
 / ___| _ __ ___   ___   __ _      |  _ \ ___
 \___ \| '_ ` _ \ / _ \ / _` |_____| |_) / __|
  ___) | | | | | | (_) | (_| |_____|  _ <\__ \
 |____/|_| |_| |_|\___/ \__, |     |_| \_\___/
                        |___/                         "#;

const BME280_EMPTY_SAMPLE_MSG: &str = "〇 BME280 returned empty or partial data";

pub(crate) fn print_splash_screen() {
    info!("{}", SPLASH_SCREEN);
}

pub(crate) fn log_weather_data(data: &WeatherData) {
    let ts = get_formatted_timestamp();

    let env_msg = format!(
        "[ 🌡️ Temp {:.2}C | 💧Humidity {:.2}% | ☁️ Pressure {:.2} hPa ]",
        data.temperature, data.humidity, data.pressure
    );
    log_message(Level::Info, &env_msg, &ts);

    if let Some(voc) = data.voc {
        let voc_msg = format!("🍃 Indoor air quality (VOC) index: {}", voc);
        log_message(Level::Info, &voc_msg, &ts);
    }
}

pub(crate) fn log_sensor_error(sensor_name: &str, error: impl std::fmt::Debug) {
    let ts = get_formatted_timestamp();

    log_message(
        Level::Error,
        &format!("🚫 {} Error: {:?}", sensor_name, error),
        &ts,
    );
}

pub(crate) fn log_empty_sample() {
    let ts = get_formatted_timestamp();

    log_message(Level::Warn, BME280_EMPTY_SAMPLE_MSG, &ts);
}

fn log_message(level: Level, message: &str, custom_ts: &str) {
    let uptime = crate::util::time_utils::get_uptime_string();
    let prefix = format!("{} [{}]", uptime, custom_ts);

    match level {
        Level::Error => error!("\x1b[31m{} {}\x1b[0m", prefix, message),
        Level::Warn => warn!("\x1b[38;5;11m{} {}\x1b[0m", prefix, message),
        _ => info!("\x1b[38;5;40m{} {}\x1b[0m", prefix, message),
    }
}
