use crate::config::{TIMESTAMP_PATTERN, TIMEZONE};
use anyhow::Context;
use chrono::Utc;
use chrono_tz::Tz;
use embassy_time::{Duration, Timer};
use esp_idf_svc::sntp::{EspSntp, SyncStatus};
use esp_idf_svc::sys::esp_timer_get_time;
use log::{info, warn};

pub(crate) async fn setup_ntp() -> anyhow::Result<EspSntp<'static>> {
    let ntp_client = EspSntp::new_default().context("‼️ Failed to init NTP")?;
    info!("\x1b[38;5;27m ⏳ Time sync in progress...");

    let mut wait_cycles = 0;
    const MAX_WAIT_CYCLES: u32 = 500;

    while ntp_client.get_sync_status() != SyncStatus::Completed {
        if wait_cycles >= MAX_WAIT_CYCLES {
            warn!(
                "\x1b[38;5;11m ⏳ NTP sync timed out. Proceeding with system time (sync will continue in background)."
            );
            anyhow::bail!("‼️ ⏳ NTP sync timed out")
        }

        Timer::after(Duration::from_millis(100)).await;

        wait_cycles += 1;
    }

    info!("\x1b[38;5;27m ⏳ Time is synchronized");
    Ok(ntp_client)
}

pub(crate) fn get_timestamp() -> String {
    let tz: Tz = TIMEZONE.parse().unwrap_or(chrono_tz::UTC);
    let now = Utc::now().with_timezone(&tz);
    now.format(TIMESTAMP_PATTERN).to_string()
}

pub(crate) fn get_uptime_string() -> String {
    let micros = unsafe { esp_timer_get_time() };
    let seconds = micros / 1_000_000;
    let millis = (micros % 1_000_000) / 1_000;
    format!("[{:>4}.{:03}s]", seconds, millis)
}
