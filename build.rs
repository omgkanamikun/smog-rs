use dotenvy::dotenv_iter;
use embuild::espidf;

fn main() {
    load_dotenv_variables();
    espidf::sysenv::output();
}

/// Bridges the gap between the host machine's environment and the ESP32 target.
///
/// Since microcontrollers do not have a traditional file system to read `.env` files
/// at runtime, we must "bake" these secrets into the binary during compilation.
///
/// This function uses the Cargo Communication Protocol:
/// 1. It reads key-value pairs from the local `.env` file via `dotenvy`.
/// 2. It emits `cargo:rustc-env=KEY=VALUE` instructions to the console.
/// 3. Cargo intercepts these instructions and provides them to the `rustc` compiler.
/// 4. The `env!("KEY")` macro in `main.rs` can then access these values and
///    hard-code them into the final machine code.
///
/// # Security Note
/// This method hard-codes secrets into the firmware image. For commercial products,
/// consider using ESP-IDF's **NVS (Non-Volatile Storage)** or **Wi-Fi Provisioning**
/// to allow users to set credentials without re-flashing.
fn load_dotenv_variables() {
    // To ensure the build script re-runs if the secrets change
    println!("cargo:rerun-if-changed=.env");

    if let Ok(iter) = dotenv_iter() {
        for item in iter {
            let (key, value) = item.expect("Failed to read .env element");
            println!("cargo:rustc-env={}={}", key, value);
        }
    }
}
