use anyhow::Context;
use bme280_rs::{Bme280, Configuration, Oversampling, SensorMode};
use chrono::Local;
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::log::EspLogger;
// use esp_idf_svc::sntp::{EspSntp, SyncStatus};
use esp_idf_svc::sys;
use log::{error, info, warn};
use sgp40::Sgp40;
use std::cell::RefCell;
use sys::esp_timer_get_time;
use sys::link_patches;

const DELAY: u32 = 4000;
const TIMESTAMP_PATTERN: &str = "%Y-%m-%d %H:%M:%S";
const UNEXPECTED_PROCESS_DELAY: &str = "Process took longer than the expected threshold";

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

    // let ntp_client = EspSntp::new_default()
    //     .with_context(|| "Failed to init NTP")?;
    // info!("\x1b[38;5;27m Синхронізація часу через NTP...");
    //
    // while ntp_client.get_sync_status() != SyncStatus::Completed {
    //     FreeRtos::delay_ms(100);
    // }
    // info!("\x1b[38;5;27m Час синхронізовано! Тепер логи будуть з реальною датою.");

    let peripherals = Peripherals::take().with_context(|| "Failed to take Peripherals")?;

    // to disable the 'Lighthouse'
    let mut led_data_pin = PinDriver::output(peripherals.pins.gpio8)
        .with_context(|| "Failed to initialize PinDriver")?;
    led_data_pin.set_low()?;

    let i2c_controller = peripherals.i2c0;
    let sda_data_pin = peripherals.pins.gpio6;
    let scl_clock_pin = peripherals.pins.gpio7;

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

    // https://talyian.github.io/ansicolors/
    info!("\x1b[38;5;27m✅ Sensors initialized successfully!\x1b[0m");

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
                    let uptime = get_uptime_string();
                    let timestamp = get_timestamp();
                    info!(
                        "\x1b[38;5;40m {} [{}] [ 🌡️  Температура: {:.2}C | 💧 Вологість: {:.2}% | ☁️  Тиск: {:.2} hPa ]",
                        uptime,
                        timestamp,
                        temperature,
                        humidity,
                        pressure / 100.0 // Pressure is usually in Pa, convert to hPa
                    );

                    // A tiny delay between sensors to let the I2C bus settle
                    FreeRtos::delay_ms(10);

                    match sgp.measure_voc_index_with_rht(humidity as u16, temperature as i16) {
                        Ok(voc) => log_sensor_data("info", &format!("🍃 VOC Index: {}", voc)),
                        Err(e) => log_sensor_data("error", &format!("⚠️  SGP40 Error: {:?}", e)),
                    }
                } else {
                    log_sensor_data("warn", UNEXPECTED_PROCESS_DELAY);
                }
            }
            Err(e) => log_sensor_data("error", &format!("🚫 BME280 Error: {:?}", e)),
        }

        FreeRtos::delay_ms(DELAY);
    }
}

fn log_sensor_data(level: &str, message: &str) {
    let uptime = get_uptime_string();
    let timestamp = get_timestamp();
    let prefix = format!("{} [{}]", uptime, timestamp);

    match level {
        "error" => error!("\x1b[31m{} {}\x1b[0m", prefix, message),
        "warn" => warn!("\x1b[33m{} {}\x1b[0m", prefix, message),
        _ => info!("\x1b[38;5;40m{} {}\x1b[0m", prefix, message),
    }
}

fn get_uptime_string() -> String {
    let micros = unsafe { esp_timer_get_time() };
    let seconds = micros / 1_000_000;
    let millis = (micros % 1_000_000) / 1_000;
    format!("[{:>4}.{:03}s]", seconds, millis)
}

fn get_timestamp() -> String {
    let now = Local::now();
    now.format(TIMESTAMP_PATTERN).to_string()
}
