#![feature(const_cmp)]
mod config;
mod logging;
mod models;
mod network;
mod sensors;
mod tasks;
mod time_utils;

use crate::config::I2C_BAUDRATE_HERTZ;
use crate::sensors::WeatherStation;
use anyhow::{Context, anyhow};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::{Gpio8, Output, PinDriver};
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::link_patches;
use log::{error, info};
use std::cell::RefCell;

type SharedI2cBus = RefCell<I2cDriver<'static>>;
type I2cBusDevice = RefCellDevice<'static, I2cDriver<'static>>;

async fn run(spawner: Spawner) -> anyhow::Result<()> {
    logging::print_splash_screen();

    let peripherals = Peripherals::take().context("Failed to take Peripherals")?;
    let _lighthouse_guard = disable_lighthouse(peripherals.pins.gpio8)?;

    let system_event_loop = EspSystemEventLoop::take()?;
    let non_volatile_storage = EspDefaultNvsPartition::take()?;

    let _wifi_guard =
        network::setup_wifi(peripherals.modem, system_event_loop, non_volatile_storage).await;
    let _ntp_guard = time_utils::setup_ntp().await?;

    let i2c_controller = peripherals.i2c0;
    let serial_data_pin = peripherals.pins.gpio6;
    let serial_clock_pin = peripherals.pins.gpio7;

    let i2c_driver = I2cDriver::new(
        i2c_controller,
        serial_data_pin,
        serial_clock_pin,
        &I2cConfig::new().baudrate(Hertz::from(I2C_BAUDRATE_HERTZ)),
    )
    .context("‼️ Failed to initialize I2C Driver")?;

    let i2c_shared_bus = Box::leak(Box::new(RefCell::new(i2c_driver)));

    let station = WeatherStation::new(i2c_shared_bus).context("☔️ WS init error")?;
    let static_station = Box::leak(Box::new(station));

    info!("\x1b[38;5;27m✅ Sensors initialized successfully!\x1b[0m");

    Timer::after(Duration::from_millis(1000)).await;

    spawner
        .spawn(tasks::network_task())
        .map_err(|_| anyhow!("‼️ Failed to spawn network task"))?;

    spawner
        .spawn(tasks::sensor_task(static_station))
        .map_err(|_| anyhow!("‼️ Failed to spawn sensor task"))?;

    // IMPORTANT: The run function must not end immediately,
    // or the Wi-Fi/NTP resources might be dropped.
    loop {
        Timer::after(Duration::from_secs(86400)).await;
    }
}

fn disable_lighthouse(gpio_pin: Gpio8) -> anyhow::Result<PinDriver<'static, Gpio8, Output>> {
    let mut led_data_pin_driver =
        PinDriver::output(gpio_pin).context("Failed to initialize PinDriver")?;
    led_data_pin_driver.set_low()?;
    Ok(led_data_pin_driver)
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    link_patches();
    EspLogger::initialize_default();

    if let Err(e) = run(spawner).await {
        error!("‼️ Fatal error during execution: {:?}", e);
    }
}
