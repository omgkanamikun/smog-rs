pub(crate) struct WeatherData {
    pub(crate) temperature: f32,
    pub(crate) humidity: f32,
    pub(crate) pressure: f32,
    pub(crate) voc: Option<u16>,
    pub(crate) timestamp: String,
}
