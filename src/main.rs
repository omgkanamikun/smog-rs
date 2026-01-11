use anyhow::{anyhow, Context};
use bme280_rs::{Bme280, Configuration, Oversampling, SensorMode};
use chrono::Local;
use chrono_tz::Europe::Warsaw;
use embassy_executor::Spawner;
use embassy_time::Delay;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::{Gpio8, Output, PinDriver};
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{EspSntp, SyncStatus};
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration as WifiConfig, EspWifi};
use log::{error, info, warn};
use sgp40::Sgp40;
use std::cell::RefCell;
use sys::esp_timer_get_time;
use sys::link_patches;
use LogLevel::{Error, Warn};

type SharedI2cBus = RefCell<I2cDriver<'static>>;
type I2cBusDevice = RefCellDevice<'static, I2cDriver<'static>>;

const WIFI_SSID: &str = env!("WIFI_2GZ_SSID");
const WIFI_PASS: &str = env!("WIFI_2GZ_PASS");
const EXECUTION_DELAY: u64 = 1000;
const TIMESTAMP_PATTERN: &str = "%Y-%m-%d %H:%M:%S";
const BME280_EMPTY_SAMPLE: &str = "\x1b[38;5;11m BME280 returned empty or partial data";

enum LogLevel {
    Info,
    Warn,
    Error,
}

struct WeatherStation {
    bme280: Bme280<I2cBusDevice, Delay>,
    sgp40: Sgp40<I2cBusDevice, Delay>,
}

impl WeatherStation {
    fn new(i2c_bus: &'static SharedI2cBus) -> anyhow::Result<Self> {
        let bme_i2c = RefCellDevice::new(i2c_bus);
        let sgp_i2c = RefCellDevice::new(i2c_bus);

        let mut bme = Bme280::new(bme_i2c, Delay);
        bme.init().context("Failed to init BME280")?;

        let bme_sampling_config = Configuration::default()
            .with_humidity_oversampling(Oversampling::Oversample1)
            .with_temperature_oversampling(Oversampling::Oversample1)
            .with_pressure_oversampling(Oversampling::Oversample1)
            .with_sensor_mode(SensorMode::Normal);
        bme.set_sampling_configuration(bme_sampling_config)
            .context("BME280 sensor configuration error")?;

        let sgp = Sgp40::new(sgp_i2c, 0x59, Delay);

        Ok(Self {
            bme280: bme,
            sgp40: sgp,
        })
    }

    async fn update(&mut self) {
        match self.bme280.read_sample() {
            Ok(sample) => {
                if let (Some(t), Some(h), Some(p)) =
                    (sample.temperature, sample.humidity, sample.pressure)
                {
                    Timer::after(Duration::from_millis(50)).await;

                    let voc = match self.sgp40.measure_voc_index_with_rht(
                        h.round().clamp(0.0, 100.0) as u16,
                        t.round().clamp(-40.0, 85.0) as i16,
                    ) {
                        Ok(voc_index) => Some(voc_index),
                        Err(sgp_error) => {
                            self.log_generic(
                                Error,
                                &format!("🚫 SGP40 Measuring Error: {:?}", sgp_error),
                                None,
                            );
                            None
                        }
                    };

                    let data = WeatherData {
                        temperature: t,
                        humidity: h,
                        pressure: p / 100.0, // Standard conversion to hPa
                        voc,
                        timestamp: get_timestamp(),
                    };
                    self.log_reading(data);
                } else {
                    self.log_generic(Warn, BME280_EMPTY_SAMPLE, None);
                }
            }
            Err(e) => self.log_generic(Error, &format!("🚫 BME280 Error: {:?}", e), None),
        }
    }

    fn log_reading(&self, data: WeatherData) {
        let env_msg = format!(
            "[ 🌡️ Temp {:.2}C | 💧Humidity {:.2}% | ☁️ Pressure {:.2} hPa ]",
            data.temperature, data.humidity, data.pressure
        );
        self.log_generic(LogLevel::Info, &env_msg, Some(&data.timestamp));

        if let Some(voc) = data.voc {
            let voc_msg = format!("🍃 Indoor air quality (VOC) index: {}", voc);
            self.log_generic(LogLevel::Info, &voc_msg, Some(&data.timestamp));
        }
    }

    fn log_generic(&self, level: LogLevel, message: &str, custom_ts: Option<&str>) {
        let uptime = get_uptime_string();
        let ts = custom_ts
            .map(|s| s.to_string())
            .unwrap_or_else(get_timestamp);
        let prefix = format!("{} [{}]", uptime, ts);

        match level {
            Error => error!("\x1b[31m{} {}\x1b[0m", prefix, message),
            Warn => warn!("\x1b[38;5;11m{} {}\x1b[0m", prefix, message),
            LogLevel::Info => info!("\x1b[38;5;40m{} {}\x1b[0m", prefix, message),
        }
    }
}

struct WeatherData {
    temperature: f32,
    humidity: f32,
    pressure: f32,
    voc: Option<u16>,
    timestamp: String,
}

fn print_splash_screen() {
    info!(
        "\n  ____                              ____      \n / ___| _ __ ___   ___   __ _      |  _ \\ ___ \n \\___ \\| '_ ` _ \\ / _ \\ / _` |_____| |_) / __|\n  ___) | | | | | | (_) | (_| |_____|  _ <\\__ \\\n |____/|_| |_| |_|\\___/ \\__, |     |_| \\_\\___/\n                        |___/                         "
    );
}

fn disable_lighthouse(gpio_pin: Gpio8) -> anyhow::Result<PinDriver<'static, Gpio8, Output>> {
    let mut led_data_pin_driver =
        PinDriver::output(gpio_pin).context("Failed to initialize PinDriver")?;
    led_data_pin_driver.set_low()?;
    Ok(led_data_pin_driver)
}

async fn setup_wifi(
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> anyhow::Result<EspWifi<'static>> {
    let mut wifi = EspWifi::new(modem, sys_loop, Some(nvs))?;
    wifi.set_configuration(&WifiConfig::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().expect("SSID is too long"),
        password: WIFI_PASS.try_into().expect("Password is too long"),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;
    wifi.start()?;
    info!("📶 WiFi starting...");

    Timer::after(Duration::from_millis(500)).await;

    let mut attempts = 0;
    loop {
        attempts += 1;
        info!("📶 WiFi connecting (attempt {})...", attempts);
        match wifi.connect() {
            Ok(_) => {
                let mut wait_counter = 0;

                while !wifi.is_connected()? {
                    Timer::after(Duration::from_millis(250)).await;

                    wait_counter += 1;
                    if wait_counter > 40 {
                        break;
                    }
                }

                if wifi.is_connected()? {
                    break;
                }
            }
            Err(e) => warn!("📶 Connect call failed: {:?}", e),
        }

        if attempts >= 5 {
            anyhow::bail!("📶 Failed to connect after {} attempts", attempts);
        }

        info!("📶 Connection refused or timed out, retrying in 2s...");
        Timer::after(Duration::from_millis(2000)).await;
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;
    info!("📶 WiFi Connected! IP: {}", ip_info.ip);

    Ok(wifi)
}

async fn setup_ntp() -> anyhow::Result<EspSntp<'static>> {
    let ntp_client = EspSntp::new_default().context("‼️ Failed to init NTP")?;
    info!("\x1b[38;5;27m ⏳Time sync in progress...");

    let mut wait_cycles = 0;
    const MAX_WAIT_CYCLES: u32 = 500;

    while ntp_client.get_sync_status() != SyncStatus::Completed {
        if wait_cycles >= MAX_WAIT_CYCLES {
            warn!(
                "\x1b[38;5;11m ⏳ NTP sync timed out. Proceeding with system time (sync will continue in background)."
            );
            anyhow::bail!("‼️⏳ NTP sync timed out")
        }

        Timer::after(Duration::from_millis(100)).await;

        wait_cycles += 1;
    }

    info!("\x1b[38;5;27m ⏳Time is syncronised");
    Ok(ntp_client)
}

fn get_uptime_string() -> String {
    let micros = unsafe { esp_timer_get_time() };
    let seconds = micros / 1_000_000;
    let millis = (micros % 1_000_000) / 1_000;
    format!("[{:>4}.{:03}s]", seconds, millis)
}

fn get_timestamp() -> String {
    let now = Local::now().with_timezone(&Warsaw);
    now.format(TIMESTAMP_PATTERN).to_string()
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    link_patches();
    EspLogger::initialize_default();

    if let Err(e) = run(spawner).await {
        error!("‼️ Fatal error during execution: {:?}", e);
    }
}

async fn run(spawner: Spawner) -> anyhow::Result<()> {
    print_splash_screen();

    let peripherals = Peripherals::take().context("Failed to take Peripherals")?;
    let _lighthouse_guard = disable_lighthouse(peripherals.pins.gpio8)?;

    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let _wifi_guard = setup_wifi(peripherals.modem, sys_loop, nvs).await;

    let _ntp_guard = setup_ntp().await?;

    let i2c_controller = peripherals.i2c0;
    let serial_data_pin = peripherals.pins.gpio6;
    let serial_clock_pin = peripherals.pins.gpio7;

    let i2c_driver = I2cDriver::new(
        i2c_controller,
        serial_data_pin,
        serial_clock_pin,
        &I2cConfig::new().baudrate(Hertz::from(100_000)),
    )
    .context("‼️ Failed to initialize I2C Driver")?;

    let i2c_shared_bus = Box::leak(Box::new(RefCell::new(i2c_driver)));

    let station = WeatherStation::new(i2c_shared_bus).context("WS init error")?;
    let static_station = Box::leak(Box::new(station));

    info!("\x1b[38;5;27m✅ Sensors initialized successfully!\x1b[0m");

    Timer::after(Duration::from_millis(1000)).await;

    spawner
        .spawn(sensor_task(static_station))
        .map_err(|_| anyhow!("‼️ Failed to spawn sensor task"))?;

    // IMPORTANT: The run function must not end immediately,
    // or the Wi-Fi/NTP resources might be dropped.
    loop {
        Timer::after(Duration::from_millis(3600)).await;
    }
}

#[embassy_executor::task]
async fn sensor_task(station: &'static mut WeatherStation) {
    loop {
        station.update().await;
        Timer::after(Duration::from_millis(EXECUTION_DELAY)).await;
    }
}
