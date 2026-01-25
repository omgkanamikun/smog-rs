# smog-rs 🦀🌬️

High-performance environmental monitoring firmware for the **ESP32-C3**, written in **Rust**.

`smog-rs` turns an ESP32-C3 into a smart weather station, providing real-time data on temperature, humidity, atmospheric pressure, and Volatile Organic Compounds (VOC Index) using the Bosch BME280 and Sensirion SGP40 sensors. Data can be logged to the terminal and optionally reported to a remote HTTP endpoint.

## ✨ Features

- **Asynchronous Execution**: Powered by `embassy-executor` for efficient multitasking on the ESP32.
- **Robust I2C Management**: Uses `embedded-hal-bus` with `RefCell` to safely share a single I2C bus between multiple sensors (BME280 and SGP40).
- **Resilient Wi-Fi**: Implements a proactive connection manager with retry logic specifically tuned for unstable routers.
- **Time Sync (SNTP)**: Automatically synchronizes with global NTP servers (configured for Europe/Warsaw) on boot for accurate data timestamping.
- **HTTP Reporting**: Support for sending sensor data to a JSON endpoint with configurable intervals.
- **Professional Logging**: Color-coded ANSI terminal output with microsecond-accurate uptime tracking and formatted timestamps.
- **SGP40 Self-Healing**: Detects the SGP40 "stuck at `VOC=1`" condition (after warm-up) and triggers a controlled MCU reboot to recover automatically.

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

The app uses environment variables at **compile time**. Create a `.env` file in the root directory by copying the template:

```bash
cp .env.example .env
```

Edit `.env` with your actual credentials:

```dotenv
# WiFi Credentials
WIFI_2GZ_SSID=your_ssid
WIFI_2GZ_PASS=your_password

# HTTP Reporting Configuration
HTTP_SENDING_ENABLED=true
HTTP_CONSUMER_ENDPOINT_URL=https://your-api-endpoint.com/data

# Localization
TIMEZONE=Europe/Warsaw
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

## 🛠️ Development Workflow (Justfile)

This project includes a `Justfile` to simplify common development tasks. If you have [`just`](https://github.com/casey/just) installed, you can use the following commands:

| Command        | Description                                                                            |
|:---------------|:---------------------------------------------------------------------------------------|
| `just setup`   | Full environment setup: installs tools, initializes ESP toolchain, and creates `.env`. |
| `just build`   | Compiles the project in release mode.                                                  |
| `just run`     | **Recommended**: Builds, flashes, and starts the serial monitor.                       |
| `just flash`   | Flashes the pre-compiled release binary to the device.                                 |
| `just monitor` | Opens the serial monitor for an already flashed device.                                |
| `just clean`   | Removes build artifacts.                                                               |

*To see all available commands, simply run `just`.*

## ⚙️ Optimization Profile

The `release` profile in `Cargo.toml` is configured for high performance and minimal binary size:

- **`opt-level = "s"`**: Optimizes for binary size while maintaining good performance.
- **`lto = "fat"`**: Enables Link-Time Optimization across all dependencies for maximum dead-code elimination.
- **`codegen-units = 1`**: Increases optimization potential by treating the crate as a single unit.
- **`panic = "abort"`**: Reduces binary size by removing stack unwinding code.
- **`strip = true`**: Removes debug symbols from the ELF file (note: this reduces file size on disk, not the size of the flashed image).

> [!TIP]
> **Debugging:** If you need to debug a release build with a debugger or detailed backtraces, you should set `strip = false` and `codegen-units = 16` (or comment them out) in `Cargo.toml` to preserve symbol information and speed up compilation.

## 📊 Data Model

The app sends a JSON payload to the configured endpoint:

```json
{
  "temperature": 22.45,
  "humidity": 45.12,
  "pressure": 1013.25,
  "voc": 105,
  "time_synced": true,
  "timestamp_unix_s": 1736376930,
  "timezone": "Europe/Warsaw"
}
```

### Timestamp semantics

- `timestamp_unix_s` is **Unix epoch seconds (UTC)** (an absolute moment in time).
- `timezone` is an **IANA timezone identifier** used for display/localization (e.g. `"Europe/Warsaw"`).
- `time_synced` indicates whether SNTP has synchronized the device clock. If `false`, consumers may prefer using ingestion time (`received_at`) or storing the sample as “unsynced” until a valid clock is available.

## 🛠️ Architecture & Design Patterns

- **Static Promotion**: Hardware drivers and the `WeatherStation` are promoted to `'static` via `Box::leak`. This is a common pattern in embedded Rust to simplify sharing resources across async tasks without a complex lifetime or `Arc` overhead.
- **Channel-based Communication**: The `sensor_task` produces data and sends it through an `embassy_sync::channel`, which the `network_task` consumes. This decouples sensing frequency from network latency.
- **Resilience**: The `network_task` implements a "Phoenix" pattern where the entire `HttpClient` is dropped and recreated if a request fails. This clears any "poisoned" internal states in the underlying ESP-IDF HTTP stack.
- **Shared Bus**: `RefCellDevice` from `embedded-hal-bus` allows safe, synchronous access to the I2C peripheral from multiple drivers within the same executor.
- **SGP40 Recovery Supervisor**:
  - The firmware tracks SGP40 behavior after a warm-up window.
  - If the VOC index remains `1` for a configurable number of consecutive samples, the sensor task requests a reboot using an Embassy `Signal`.
  - A dedicated `reboot_supervisor_task` performs the restart (`esp_restart()`), keeping reboot logic centralized and reducing complexity in the sensor loop.

## 📜 License
MIT

