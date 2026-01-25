use crate::config::TIMEZONE;
use crate::logging::{log_empty_sample, log_sensor_error};
use crate::models::WeatherData;
use crate::{I2cBusDevice, SharedI2cBus, time_utils};
use anyhow::Context;
use bme280_rs::{Bme280, Configuration, Oversampling, SensorMode};
use embassy_time::{Delay, Duration, Instant, Timer};
use embedded_hal_bus::i2c::RefCellDevice;
use sgp40::Sgp40;

const SGP_40_WARMUP_SECS: u64 = 60;
const SGP_40_STUCK_AT_ONE_THRESHOLD: u16 = 20;

pub(crate) struct WeatherStation {
    bme280: Bme280<I2cBusDevice, Delay>,
    sgp40: Sgp40<I2cBusDevice, Delay>,
    sgp40health: Sgp40Health,
}

impl WeatherStation {
    pub(crate) fn new(i2c_bus: &'static SharedI2cBus) -> anyhow::Result<Self> {
        let bme_i2c = RefCellDevice::new(i2c_bus);
        let sgp_i2c = RefCellDevice::new(i2c_bus);

        let mut bme = Bme280::new(bme_i2c, Delay);

        bme.init().context("‼️Failed to init BME280")?;

        let bme_sampling_config = Configuration::default()
            .with_humidity_oversampling(Oversampling::Oversample1)
            .with_temperature_oversampling(Oversampling::Oversample1)
            .with_pressure_oversampling(Oversampling::Oversample1)
            .with_sensor_mode(SensorMode::Normal);

        bme.set_sampling_configuration(bme_sampling_config)
            .context("‼️BME280 sensor configuration error")?;

        let sgp = Sgp40::new(sgp_i2c, 0x59, Delay);
        let sgp40health = Sgp40Health::new();

        Ok(Self {
            bme280: bme,
            sgp40: sgp,
            sgp40health,
        })
    }

    pub(crate) async fn read_sensor_data(&mut self) -> Option<WeatherData> {
        match self.bme280.read_sample() {
            Ok(sample) => {
                if let (Some(t), Some(h), Some(p)) =
                    (sample.temperature, sample.humidity, sample.pressure)
                {
                    Timer::after_millis(50).await;

                    let voc = match self.sgp40.measure_voc_index_with_rht(
                        h.round().clamp(0.0, 100.0) as u16,
                        t.round().clamp(-40.0, 85.0) as i16,
                    ) {
                        Ok(voc_index) => Some(voc_index),
                        Err(sgp_error) => {
                            log_sensor_error("SGP40 Measuring", sgp_error);
                            None
                        }
                    };

                    Some(WeatherData {
                        temperature: t,
                        humidity: h,
                        pressure: p / 100.0, // Standard conversion to hPa
                        voc,
                        time_synced: time_utils::is_time_synced(),
                        timestamp_unix_s: time_utils::timestamp_unix_s(),
                        timezone: TIMEZONE,
                    })
                } else {
                    log_empty_sample();
                    None
                }
            }
            Err(e) => {
                log_sensor_error("BME280", e);
                None
            }
        }
    }

    pub(crate) fn sgp40_stuck_at_one(&mut self, voc: Option<u16>) -> bool {
        self.sgp40health.check_stuck_condition(voc)
    }
}

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

    fn check_stuck_condition(&mut self, voc: Option<u16>) -> bool {
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
