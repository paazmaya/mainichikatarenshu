use anyhow::Ok;

use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::{prelude::*, text::Text};

mod ssd1680;

pub use crate::ssd1680::cmd::Cmd;
pub use crate::ssd1680::color::Color;

pub use crate::ssd1680::driver::Ssd1680;
pub use crate::ssd1680::flag::Flag;
pub use crate::ssd1680::pins::Pins;

pub use crate::ssd1680::graphics::{Display, Display2in13, DisplayRotation};
// https://docs.rs/embedded-graphics/0.8.1/embedded_graphics/mono_font/index.html#modules
use embedded_graphics::mono_font::{
    iso_8859_15::FONT_10X20 as ISO15_10, MonoTextStyle, MonoTextStyleBuilder,
};

use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::peripherals::Peripherals;

use esp_idf_svc::hal::gpio;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::spi;

// Include the pre-converted logo image binary data (generated at build time)
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
fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().expect("Could not take peripherals");
    let pins = peripherals.pins;

    // Set the RTC to current date/time (you can update this to the actual current date)
    // Format: year, month (1-12), day, hour (0-23), minute (0-59), second (0-59)
    log::info!("Setting RTC to current date and time");
    set_rtc_datetime(2025, 10, 14, 18, 0, 0); // October 14, 2025, 1:00 PM

    // Read back the date to verify
    let current_date = get_rtc_date();
    log::info!("Current RTC date: {}", current_date);

    // Configure SPI to match Arduino example exactly
    log::info!("Configuring SPI with Arduino-compatible settings");
    let mut driver = spi::SpiDeviceDriver::new_single(
        peripherals.spi2,
        pins.gpio12,                                          // SCK - Pins::SCK
        pins.gpio11,                                          // MOSI - Pins::MOSI
        Option::<gpio::AnyIOPin>::None,                       // No MISO needed for display
        Some(pins.gpio45),                                    // CS - Pins::CS
        &spi::SpiDriverConfig::new().dma(spi::Dma::Disabled), // Disable DMA to match Arduino's bit-banging
        &spi::SpiConfig::new().baudrate(200.kHz().into()), // Very low speed to match Arduino's bit-banging
                                                           // Note: Mode0 is default in esp-idf-hal, so we don't need to set it explicitly
    )
    .expect("Could not create SPI device driver");

    // Enable display power explicitly - this is done in the Arduino example
    log::info!("Enabling display power (pin 7)");
    let mut power_pin = gpio::PinDriver::output(pins.gpio7).expect("Failed to set pin 7 as output");
    power_pin.set_high().expect("Failed to set power pin high");

    // Create delay for timing
    let delay = Delay::default();
    delay.delay_ms(100); // Wait for power to stabilize

    // Create driver with standard initialization first
    log::info!("Creating display driver with standard init");
    let mut ssd1680 = Ssd1680::new(
        &mut driver,
        gpio::PinDriver::input(pins.gpio48).expect("Failed to set busy pin as input"), // Pins::BSY
        gpio::PinDriver::output(pins.gpio46).expect("Failed to set dc pin as output"), // Pins::DC
        gpio::PinDriver::output(pins.gpio47).expect("Failed to set rst pin as output"), // Pins::RST
        delay,
    )
    .expect("Could not create EPD driver");

    // HARDWARE DIAGNOSTIC FIRST
    log::info!("======================================================");
    log::info!("STARTING HARDWARE DIAGNOSTIC TESTS");
    log::info!("======================================================");

    // Use EXACT Arduino EPD_Init() - minimal, matching Arduino exactly
    log::info!("\n\n=== EXACT ARDUINO EPD_Init() ===");
    if let Err(e) = ssd1680.cpp_init() {
        log::error!("Arduino init failed: {:?}", e);
        return Err(anyhow::anyhow!("Arduino init failed: {:?}", e));
    }
    log::info!("Arduino initialization successful");

    // Now follow EXACT Arduino sequence from 2.9_key.ino setup()
    // EPD_Init() -> EPD_ALL_Fill(WHITE) -> EPD_Update() -> EPD_Clear_R26H()
    log::info!("\n\n=== EXACT ARDUINO WORKING SEQUENCE ===");

    // Step 1-3: Complete Arduino sequence (fill -> update -> clear R26h)
    log::info!("Step 1-3: Arduino sequence with WHITE");
    if let Err(e) = ssd1680.fill_update_clear(Flag::AUTO_WRITE_PATTERN_ALL_WHITE) {
        log::error!("Failed Arduino sequence with white: {:?}", e);
    }

    log::info!("Arduino sequence complete. Display should show WHITE. Waiting 5 seconds...");
    delay.delay_ms(5000);

    // Now try with BLACK - this should make the screen turn black
    log::info!("Filling display with BLACK");
    if let Err(e) = ssd1680.fill_update_clear(Flag::AUTO_WRITE_PATTERN_ALL_BLACK) {
        log::error!("Failed Arduino sequence with black: {:?}", e);
    }

    log::info!("Display should be COMPLETELY BLACK");
    log::info!("Waiting 3 seconds...");
    delay.delay_ms(3000);

    // Clear to white first
    log::info!("Clearing display to white");
    if let Err(e) = ssd1680.fill_update_clear(Flag::AUTO_WRITE_PATTERN_ALL_WHITE) {
        log::error!("Failed to clear to white: {:?}", e);
    }
    delay.delay_ms(500);

    // Create display buffer
    log::info!("Creating display buffer with date");
    let mut display = Display2in13::new();
    // Use Rotate270 to match the physical RAM orientation used by raw image data
    // The raw logo image is pre-rotated for physical display (128×296)
    display.set_rotation(DisplayRotation::Rotate270);

    // Clear buffer to white (Off = white for this display)
    display
        .clear(BinaryColor::Off)
        .expect("Failed to clear buffer");

    // Get current date from RTC
    let date_text = get_rtc_date();

    // Draw the date in large font
    let text_style = MonoTextStyleBuilder::new()
        .font(&ISO15_10)
        .text_color(BinaryColor::On) // On = black pixels
        .build();

    Text::new(&date_text, Point::new(10, 30), text_style)
        .draw(&mut display)
        .expect("Failed to draw date");

    // Add a label
    let label_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
    Text::new("Current Date:", Point::new(10, 10), label_style)
        .draw(&mut display)
        .expect("Failed to draw label");

    // Send buffer to display using Arduino-compatible method
    log::info!("Writing date buffer to display");
    if let Err(e) = ssd1680.write_buffer_and_update(display.buffer()) {
        log::error!("Failed to write and update buffer: {:?}", e);
        return Err(anyhow::anyhow!(
            "Failed to write and update buffer: {:?}",
            e
        ));
    }

    log::info!("DATE DISPLAYED - CHECK SCREEN!");
    log::info!("Should show: {}", date_text);
    delay.delay_ms(5000);

    // Now display the logo image (pre-converted at build time)

    if LOGO_IMAGE.is_empty() {
        log::warn!("Logo image not available (logo.png not found at build time)");
        log::warn!("Skipping logo display...");
    } else {
        log::info!("Logo image embedded, size: {} bytes", LOGO_IMAGE.len());

        // Clear to white first
        log::info!("Clearing to white before logo");
        if let Err(e) = ssd1680.fill_update_clear(Flag::AUTO_WRITE_PATTERN_ALL_WHITE) {
            log::error!("Failed to clear to white: {:?}", e);
        }
        delay.delay_ms(500);

        // Send image buffer to display
        log::info!("Sending logo buffer to display");
        if let Err(e) = ssd1680.write_buffer_and_update(LOGO_IMAGE) {
            log::error!("Failed to write and update logo: {:?}", e);
        } else {
            log::info!("LOGO DISPLAYED - CHECK SCREEN!");
        }

        delay.delay_ms(10000);
    }

    delay.delay_ms(3000);

    let wakeup_reason = esp_idf_svc::hal::reset::WakeupReason::get();
    log::info!("Wakeup reason: {:?}", wakeup_reason);

    let reset_reason = esp_idf_svc::hal::reset::ResetReason::get();
    log::info!("Reset reason: {:?}", reset_reason);

    /*

    thread::sleep(time::Duration::from_millis(1000));

    let sleep_micros = 2_000_000;
    unsafe {
        esp_idf_svc::sys::esp_sleep_enable_timer_wakeup(sleep_micros);

        log::info!("Going to deep sleep for {} seconds", sleep_micros / 1_000_000);
        esp_idf_svc::sys::esp_deep_sleep_start();
        // Software reset!
    }
    */
    Ok(())
}

/*
fn main_k() -> ! {
    //println!("wakeup: {:?}", esp_hal::reset::wakeup_cause());

    // Initialize with the highest possible frequency for this chip
    // https://docs.esp-rs.org/esp-hal/esp-hal/0.22.0/esp32s3/esp_hal/peripherals/index.html
    let peripherals = init({
        let mut config: esp_hal::Config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    //esp_alloc::heap_allocator!(72 * 1024);

    let mut delay: Delay = Delay::new();

    //let epd = create_epd_driver(&peripherals, &delay)?;

    delay.delay(500.millis());

    // Use "exit" button to wake up
    let wakeup_pin: AnyPin = peripherals.GPIO1.degrade();
    let sleep_time: Duration = Duration::from_secs(5);

    //let mut cfg = RtcSleepConfig::deep();
    //cfg.set_rtc_fastmem_pd_en(false);
    //let wakeup_source = TimerWakeupSource::new(sleep_time);
    //let mut rtc = Rtc::new(peripherals.LPWR);
    //rtc.rwdt.enable();

    delay.delay(500.millis());

    //rtc.sleep(&cfg, &[&wakeup_source]);

    loop {}
}
*/
/*
// Go look at
// https://github.com/esp-rs/esp-idf-svc/blob/master/examples/sd_spi.rs
fn connect_to_sdcard((peripherals: &Peripherals) -> ! {
    let cs = peripherals.pins.gpio10;
}
*/
