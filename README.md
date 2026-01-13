# smog-rs 🦀🌬️

A high-performance environmental monitoring firmware for the **ESP32-C3**, written in **Rust**.

`smog-rs` turns an ESP32-C3 into a smart weather station, providing real-time data on temperature, humidity, atmospheric pressure, and Volatile Organic Compounds (VOC Index) using the Bosch BME280 and Sensirion SGP40 sensors. Data can be logged to the terminal and optionally reported to a remote HTTP endpoint.

## ✨ Features

- **Asynchronous Execution**: Powered by `embassy-executor` for efficient multitasking on the ESP32.
- **Robust I2C Management**: Uses `embedded-hal-bus` with `RefCell` to safely share a single I2C bus between multiple sensors (BME280 and SGP40).
- **Resilient Wi-Fi**: Implements a proactive connection manager with retry logic specifically tuned for unstable routers.
- **"Phoenix" Network Strategy**: Automatic recovery from HTTP client internal errors by recreating the client on failure, bypassing known `esp-idf-svc` state machine quirks.
- **Time Sync (SNTP)**: Automatically synchronizes with global NTP servers (configured for Europe/Warsaw) on boot for accurate data timestamping.
- **HTTP Reporting**: Support for sending sensor data to a JSON endpoint with configurable intervals.
- **Professional Logging**: Color-coded ANSI terminal output with microsecond-accurate uptime tracking and formatted timestamps.

## 🛠️ Tech Stack

- **MCU**: ESP32-C3 (RISC-V)
- **Framework**: `esp-idf-svc` (Standard library support) + `embassy`
- **Language**: Rust (Edition 2024, nightly features like `const_cmp`)
- **Sensors**:
    - **BME280**: Temperature, Humidity, Pressure
    - **SGP40**: VOC Index (Gas sensing)

## 🚀 Getting Started

### 1. Prerequisites

Ensure you have the Rust ESP32 toolchain and necessary tools installed:

```bash
# Install espup to manage the toolchain
cargo install espup
espup install

# Install the flash and monitoring tool
cargo install espflash

# Install ldproxy if not present
cargo install ldproxy
```

### 2. Environment Configuration

The app uses environment variables at **compile time**. Create a `.env` file in the root directory:

```dotenv
# WiFi Credentials
WIFI_2GZ_SSID=your_ssid
WIFI_2GZ_PASS=your_password

# HTTP Reporting Configuration
HTTP_SENDING_ENABLED=true
HTTP_CONSUMER_ENDPOINT_URL=http://your-api-endpoint.com/data
```

*Note: Changing these values requires a re-compilation (`cargo run --release`).*

### 3. Hardware Mapping

| Sensor Pin | ESP32-C3 GPIO | Description                              |
|:-----------|:--------------|:-----------------------------------------|
| I2C SDA    | GPIO 6        | Serial Data Line (I2C)                   |
| I2C SCL    | GPIO 7        | Serial Clock Line (I2C)                  |
| Status LED | GPIO 8        | Lighthouse (Large RGB LED in the center) |

**Notes:**
- **I2C (Inter-Integrated Circuit)**: A synchronous, multi-controller/multi-target, serial communication bus. SDA and SCL are the two signals required for this protocol.
- **Lighthouse**: The ESP32-C3-Mini1 (specifically on some development boards like the ESP32-C3-DevKitM-1) features a prominent LED in the center, often referred to as the "Lighthouse" in this project's context.

### 4. Build and Flash

Connect your ESP32-C3 via USB and execute:

```bash
# Build, flash, and monitor in release mode
cargo run --release
```

## 📊 Data Model

The app sends a JSON payload to the configured endpoint:

```json
{
  "temperature": 22.45,
  "humidity": 45.12,
  "pressure": 1013.25,
  "voc": 105,
  "timestamp": "2026-01-08 21:15:30"
}
```

## 🛠️ Architecture & Design Patterns

- **Static Promotion**: Hardware drivers and the `WeatherStation` are promoted to `'static` via `Box::leak`. This is a common pattern in embedded Rust to simplify sharing resources across async tasks without complex lifetime or `Arc` overhead.
- **Channel-based Communication**: The `sensor_task` produces data and sends it through an `embassy_sync::channel`, which the `network_task` consumes. This decouples sensing frequency from network latency.
- **Resilience**: The `network_task` implements a "Phoenix" pattern where the entire `HttpClient` is dropped and recreated if a request fails. This clears any "poisoned" internal states in the underlying ESP-IDF HTTP stack.
- **Shared Bus**: `RefCellDevice` from `embedded-hal-bus` allows safe, synchronous access to the I2C peripheral from multiple drivers within the same executor.

## 📜 License
MIT / Apache-2.0
