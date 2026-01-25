use crate::config::{TIMESTAMP_PATTERN, TIMEZONE};
use anyhow::Context;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use embassy_futures::select;
use embassy_futures::select::Either;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::once_lock::OnceLock;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use esp_idf_svc::sntp::{EspSntp, SyncStatus};
use esp_idf_svc::sys::esp_timer_get_time;
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};

static TIME_SYNCED: AtomicBool = AtomicBool::new(false);
static TIME_SYNCED_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub(crate) fn is_time_synced() -> bool {
    TIME_SYNCED.load(Ordering::Relaxed)
}

pub(crate) async fn setup_ntp() -> anyhow::Result<EspSntp<'static>> {
    let ntp_client = EspSntp::new_default().context("‼️ Failed to init NTP")?;
    info!("\x1b[38;5;27m ⏳ Time sync in progress...");

    let mut wait_cycles = 0;
    const MAX_WAIT_CYCLES: u32 = 100;

    while ntp_client.get_sync_status() != SyncStatus::Completed {
        if wait_cycles >= MAX_WAIT_CYCLES {
            warn!(
                "\x1b[38;5;11m ⏳ NTP sync timed out. Proceeding with system time (sync will continue in background)."
            );
            return Ok(ntp_client);
        }

        Timer::after_millis(100).await;

        wait_cycles += 1;
    }

    mark_time_synced();

    info!("\x1b[38;5;27m ⏳ Time is synchronized");
    Ok(ntp_client)
}

pub(crate) async fn ntp_sync_watcher(ntp_client: EspSntp<'static>) {
    loop {
        if ntp_client.get_sync_status() == SyncStatus::Completed {
            if !is_time_synced() {
                info!("📡 NTP Sync Complete! Time is now valid.");
            }

            mark_time_synced();

            Timer::after_secs(60).await;
        } else {
            Timer::after_secs(1).await;
        }
    }
}

pub(crate) async fn wait_time_sync_grace_period() {
    if is_time_synced() {
        return;
    }

    match select::select(TIME_SYNCED_SIGNAL.wait(), Timer::after_secs(30)).await {
        Either::First(()) => {}
        Either::Second(()) => {
            warn!("‼️📡 NTP not synced after 30s; proceeding with time_synced=false.");
        }
    }
}

pub(crate) fn timestamp_unix_s() -> i64 {
    Utc::now().timestamp()
}

pub(crate) fn get_uptime_string() -> String {
    let micros = unsafe { esp_timer_get_time() };
    let seconds = micros / 1_000_000;
    let millis = (micros % 1_000_000) / 1_000;
    format!("[{:>4}.{:03}s]", seconds, millis)
}

pub(crate) fn get_formatted_timestamp() -> String {
    let now = get_current_time_in_timezone();
    now.format(TIMESTAMP_PATTERN).to_string()
}

fn get_current_time_in_timezone() -> DateTime<Tz> {
    Utc::now().with_timezone(cached_timezone())
}

fn cached_timezone() -> &'static Tz {
    static TZ: OnceLock<Tz> = OnceLock::new();
    TZ.get_or_init(|| TIMEZONE.parse().unwrap_or(chrono_tz::UTC))
}

fn mark_time_synced() {
    if !TIME_SYNCED.swap(true, Ordering::Relaxed) {
        TIME_SYNCED_SIGNAL.signal(())
    }
}
