use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub(crate) struct WeatherData {
    pub(crate) temperature: f32,
    pub(crate) humidity: f32,
    pub(crate) pressure: f32,
    pub(crate) voc: Option<u16>,
    pub(crate) time_synced: bool,
    pub(crate) timestamp_unix_s: i64,
    pub(crate) timezone: &'static str,
}
