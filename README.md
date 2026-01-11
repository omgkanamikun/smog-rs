# smog-rs 🦀🌬️

A high-performance environmental monitoring firmware for the **ESP32-C3**, written in **Rust**.

`smog-rs` used ESP32-C3 as a smart weather station, providing real-time data on temperature, humidity, atmospheric pressure, and Volatile Organic Compounds (VOC Index) using the Bosch BME280 and Sensirion SGP40 sensors.

## ✨ Features

- **Robust I2C Management**: Uses `embedded-hal-bus` with `RefCell` to safely share a single I2C bus between multiple sensors.
- **Resilient WiFi**: Implements a proactive connection manager with retry logic specifically tuned for picky ISP routers (e.g., Sagemcom).
- **Time Sync (SNTP)**: Automatically synchronizes with global NTP servers on boot for accurate data logging.
- **Modular Architecture**: Clean separation between hardware drivers, business logic, and reporting.
- **Professional Logging**: Color-coded ANSI terminal output with microsecond-accurate uptime tracking. See: https://talyian.github.io/ansicolors/

## 🛠️ Tech Stack

- **MCU**: ESP32-C3 (RISC-V)
- **Framework**: `esp-idf-svc` (Standard library support)
- **Language**: Rust (Edition 2024)
- **Sensors**:
    - **BME280**: Temperature, Humidity, Pressure
    - **SGP40**: VOC Index (Gas sensing)

## 🚀 Getting Started

### 1. Prerequisites

Ensure you have the Rust ESP32 toolchain installed:
```bash
cargo install espup
espup install
# Install the flash tool
cargo install espflash
```

### 2. Configuration

Create and update `.env` file in the root of the project with your Wi-Fi credentials:
```dotenv
WIFI_2GZ_SSID=YOUR_SSID
WIFI_2GZ_PASS=YOUR_PASSWORD
```

### 3. Hardware Mapping (Default)

| Sensor Pin | ESP32-C3 GPIO |
|:-----------|:--------------|
| I2C SDA    | GPIO 6        |
| I2C SCL    | GPIO 7        |
| Status LED | GPIO 8        |

### 4. Build and Flash

Connect your ESP32-C3 via USB and run:
```bash
# Build and flash in release mode for best performance
cargo run --release
```

## 📊 Terminal Output Example
```text
[   4.521s] [2026-01-08 21:15:30] [ 🌡️  22.45C | 💧 45.12% | ☁️  1013.25 hPa ]
[   4.532s] [2026-01-08 21:15:30] 🍃 VOC Index: 105
```

## 🛠️ Architecture Notes
- **Static Lifetimes**: Hardware drivers are promoted to `'static` using `Box::leak` to ensure the I2C bus remains accessible for the entire duration of the program without complex lifetime management.
- **Type Aliasing**: Extensive use of `type` aliases (e.g., `SharedI2cBus`, `I2cBusDevice`) simplifies the code and improves readability.
- **Resource Guarding**: The Status LED (GPIO8) is held in a `PinDriver` guard in `main` to prevent the hardware from resetting when the driver is dropped.

## 📝 Roadmap
- [ ] Async/Await migration using `embassy-executor`.
- [ ] MQTT integration for Home Assistant discovery.
- [ ] Deep Sleep support for battery-powered operation.

## 📜 License
MIT / Apache-2.0
