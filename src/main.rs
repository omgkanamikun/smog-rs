use anyhow::Context;
use bme280_rs::{Bme280, Configuration, Oversampling, SensorMode};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::sys;
use log::{error, info};
use sgp40::Sgp40;
use std::cell::RefCell;
use sys::link_patches;

const DELAY: u32 = 4000;

// todo: Move to async
//  The ESP32-C3 is great at async/await.
//  If you eventually want to read sensors
//  and send data over WiFi at the same time without the sensor loop blocking the WiFi,
//  look into the esp-hal-embassy or edge-executor crates
fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!(
        "\n  ____                              ____      \n / ___| _ __ ___   ___   __ _      |  _ \\ ___ \n \\___ \\| '_ ` _ \\ / _ \\ / _` |_____| |_) / __|\n  ___) | | | | | | (_) | (_| |_____|  _ <\\__ \\\n |____/|_| |_| |_|\\___/ \\__, |     |_| \\_\\___/\n                        |___/                         "
    );

    let peripherals = Peripherals::take().with_context(|| "Failed to take Peripherals")?;
    let i2c_controller = peripherals.i2c0;
    let sda_data_pin = peripherals.pins.gpio8;
    let scl_clock_pin = peripherals.pins.gpio9;

    let i2c_config = I2cConfig::new().baudrate(Hertz::from(100_000));
    let i2c_driver = I2cDriver::new(i2c_controller, sda_data_pin, scl_clock_pin, &i2c_config)
        .with_context(|| "Failed to initialize I2C Driver")?;

    let i2c_shared_bus = RefCell::new(i2c_driver);

    let bme_i2c = RefCellDevice::new(&i2c_shared_bus);
    let sgp_i2c = RefCellDevice::new(&i2c_shared_bus);

    let mut bme = Bme280::new(bme_i2c, FreeRtos);
    bme.init().with_context(|| "Failed to init BME280")?;

    // The BME280 is a sophisticated sensor that starts up in Sleep Mode by default.
    // Even though you called init(), the sensor needs to be told
    // what to measure (Oversampling) and how to run (Mode).
    let bme_sampling_config = Configuration::default()
        .with_humidity_oversampling(Oversampling::Oversample1)
        .with_temperature_oversampling(Oversampling::Oversample1)
        .with_pressure_oversampling(Oversampling::Oversample1)
        .with_sensor_mode(SensorMode::Normal);
    bme.set_sampling_configuration(bme_sampling_config)
        .with_context(|| "BME280 sensor configuration error")?;

    let mut sgp = Sgp40::new(sgp_i2c, 0x59, FreeRtos);

    info!("Sensors initialized!");

    loop {
        match bme.read_sample() {
            Ok(sample) => {
                // todo: Handling "None" better,
                //  we have to skip the SGP40 measurement completely if the BME280 fails
                let temp_option = sample.temperature;
                let hum_option = sample.humidity;
                let pr_option = sample.pressure;

                if let Some(temperature) = temp_option
                    && let Some(humidity) = hum_option
                    && let Some(pressure) = pr_option
                {
                    info!(
                        "BME280: {:.2}C, {:.2}% RH, Pressure: {:.2} hPa",
                        temperature,
                        humidity,
                        pressure / 100.0 // Pressure is usually in Pa, convert to hPa
                    );

                    // Added a tiny delay between sensors to let the I2C bus settle
                    FreeRtos::delay_ms(10);

                    match sgp.measure_voc_index_with_rht(humidity as u16, temperature as i16) {
                        Ok(voc) => info!("SGP40: VOC Index {}", voc),
                        Err(e) => error!("SGP40 Error: {:?}", e),
                    }
                }
            }
            Err(e) => error!("BME280 Error: {:?}", e),
        }

        FreeRtos::delay_ms(DELAY);
    }
}
