pub(crate) const WIFI_SSID: &str = env!("WIFI_2GZ_SSID");
pub(crate) const WIFI_PASS: &str = env!("WIFI_2GZ_PASS");
pub(crate) const EXECUTION_DELAY_MS: u64 = 1000;
pub(crate) const TIMESTAMP_PATTERN: &str = "%Y-%m-%d %H:%M:%S";
pub(crate) const BME280_EMPTY_SAMPLE_MSG: &str =
    "\x1b[38;5;11m 〇 BME280 returned empty or partial data";

pub(crate) const I2C_BAUDRATE_ESP32: u32 = 100_000;
