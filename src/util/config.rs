pub(crate) const WIFI_SSID: &str = env!("WIFI_2GZ_SSID");
pub(crate) const WIFI_PASS: &str = env!("WIFI_2GZ_PASS");
pub(crate) const HTTP_SENDING_ENABLED: &str = env!("HTTP_SENDING_ENABLED");
pub(crate) const HTTP_SEND_INTERVAL_MS: u64 = 15_000;
pub(crate) const HTTP_CONSUMER_ENDPOINT_URL: &str = env!("HTTP_CONSUMER_ENDPOINT_URL");
pub(crate) const EXECUTION_DELAY_MS: u64 = 1000;
pub(crate) const TIMESTAMP_PATTERN: &str = "%Y-%m-%d %H:%M:%S";
pub(crate) const TIMEZONE: &str = env!("TIMEZONE");
pub(crate) const I2C_BAUDRATE_HERTZ: u32 = 100_000;

pub(crate) fn is_sending_enabled() -> bool {
    HTTP_SENDING_ENABLED == "true"
}
