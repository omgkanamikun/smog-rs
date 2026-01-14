# Justfile for smog-rs

# Default recipe to show available commands
default:
    @just --list

# 1. Install necessary Rust ESP32 tools (espup, espflash, ldproxy)
install-tools:
    cargo install espup
    cargo install espflash
    cargo install ldproxy

# 2. Initialize the ESP32 toolchain using espup
init-esp:
    espup install

# 3. Setup the environment: install tools, init ESP, and create .env file
setup: install-tools init-esp
    [ -f .env ] || cp .env.example .env
    @echo ".env file created. Please update it with your credentials."

# 4. Build the project in release mode
build:
    cargo build --release

# 5. Flash the firmware to the ESP32-C3 (Optional: direct after build)
flash:
    espflash flash target/riscv32imc-esp-espidf/release/smog-rs

# 6. Build, flash, and monitor the project (Recommended: wraps build and flash)
run:
    cargo run --release

# Start the serial monitor (Direct/Optional)
monitor:
    espflash monitor

# Clean the build artifacts
clean:
    cargo clean
