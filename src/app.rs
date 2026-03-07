extern crate alloc;

use anyhow::Ok;
use alloc::string::{String, ToString};
use alloc::format;

// Import ESP32 HAL types
use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::gpio;
use esp_idf_svc::hal::spi::{SpiDeviceDriver, SpiDriverConfig, SpiConfig, Dma};
use esp_idf_svc::hal::peripherals::Peripherals;

#[cfg(test)]
mod display_tests;

const LOGO_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo.bin"));

/// Read current date and time from ESP32 RTC
/// Returns formatted ISO 8601 date string (YYYY-MM-DD)
fn get_rtc_date() -> String {
    use core::mem::MaybeUninit;
    use esp_idf_svc::sys::{localtime_r, time, time_t, tm};

    unsafe {
        // Get current time from RTC
        let mut now: time_t = 0;
        time(&mut now as *mut _);

        // Convert to local time structure
        let mut timeinfo = MaybeUninit::<tm>::uninit();
        localtime_r(&now as *const _, timeinfo.as_mut_ptr());
        let timeinfo = timeinfo.assume_init();

        // Format as ISO 8601 date (YYYY-MM-DD)
        // tm_year is years since 1900, tm_mon is 0-11
        let year = 1900 + timeinfo.tm_year;
        let month = 1 + timeinfo.tm_mon;
        let day = timeinfo.tm_mday;

        format!("{:04}-{:02}-{:02}", year, month, day)
    }
}

/// Set the ESP32 RTC to a specific date and time
/// Format: year, month (1-12), day, hour (0-23), minute (0-59), second (0-59)
fn set_rtc_datetime(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) {
    use core::mem::MaybeUninit;
    use esp_idf_svc::sys::{mktime, settimeofday, timeval, timezone, tm};

    unsafe {
        // Create time structure
        let mut timeinfo = MaybeUninit::<tm>::uninit();
        let timeinfo_ptr = timeinfo.as_mut_ptr();

        (*timeinfo_ptr).tm_year = year - 1900; // Years since 1900
        (*timeinfo_ptr).tm_mon = month - 1; // Months since January (0-11)
        (*timeinfo_ptr).tm_mday = day;
        (*timeinfo_ptr).tm_hour = hour;
        (*timeinfo_ptr).tm_min = minute;
        (*timeinfo_ptr).tm_sec = second;
        (*timeinfo_ptr).tm_isdst = -1; // Auto-detect DST

        // Convert to time_t
        let timestamp = mktime(timeinfo_ptr);

        // Set system time
        let tv = timeval {
            tv_sec: timestamp,
            tv_usec: 0,
        };

        settimeofday(&tv as *const _, core::ptr::null::<timezone>());

        log::info!(
            "RTC set to: {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year,
            month,
            day,
            hour,
            minute,
            second
        );
    }
}

// https://docs.esp-rs.org/esp-idf-svc/esp_idf_svc/
pub fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello from ESP32! Build successful!");

    // Simple delay loop
    let delay = Delay::default();
    loop {
        log::info!("Running main loop...");
        delay.delay_ms(1000);
    }
}
