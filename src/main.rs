use anyhow::Ok;

use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Line;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::text::Alignment;
use embedded_graphics::text::TextStyleBuilder;
use embedded_graphics::{prelude::*, text::Text};

mod ssd1680;

pub use crate::ssd1680::cmd::Cmd;
pub use crate::ssd1680::color::Color;
pub use crate::ssd1680::driver::Ssd1680;
pub use crate::ssd1680::flag::Flag;

pub use crate::ssd1680::graphics::{Display, Display2in13, DisplayRotation};
// https://docs.rs/embedded-graphics/0.8.1/embedded_graphics/mono_font/index.html#modules
use embedded_graphics::mono_font::{
    iso_8859_15::FONT_10X20 as ISO15_10, jis_x0201::FONT_9X15 as JIS_9, MonoTextStyle,
    MonoTextStyleBuilder,
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
        pins.gpio12,                    // SCK - same as Arduino (pin 12)
        pins.gpio11,                    // MOSI - same as Arduino (pin 11)
        Option::<gpio::AnyIOPin>::None, // No MISO needed for display
        Some(pins.gpio45),              // CS - same as Arduino (pin 45)
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
    let mut delay = Delay::default();
    delay.delay_ms(100); // Wait for power to stabilize

    // Create driver with standard initialization first
    log::info!("Creating display driver with standard init");
    let mut ssd1680 = Ssd1680::new(
        &mut driver,
        gpio::PinDriver::input(pins.gpio48).expect("Failed to set 48 busy pin as input"),
        gpio::PinDriver::output(pins.gpio46).expect("Failed to set 46 dc pin as output"),
        gpio::PinDriver::output(pins.gpio47).expect("Failed to set 47 rst pin as output"),
        &mut delay,
    )
    .expect("Could not create EPD driver");

    // HARDWARE DIAGNOSTIC FIRST
    log::info!("======================================================");
    log::info!("STARTING HARDWARE DIAGNOSTIC TESTS");
    log::info!("======================================================");

    // Use EXACT Arduino EPD_Init() - minimal, matching Arduino exactly
    log::info!("\n\n=== EXACT ARDUINO EPD_Init() ===");
    if let Err(e) = ssd1680.cpp_init(&mut delay) {
        log::error!("Arduino init failed: {:?}", e);
        return Err(anyhow::anyhow!("Arduino init failed: {:?}", e));
    }
    log::info!("Arduino initialization successful");

    // Now follow EXACT Arduino sequence from 2.9_key.ino setup()
    // EPD_Init() -> EPD_ALL_Fill(WHITE) -> EPD_Update() -> EPD_Clear_R26H()
    log::info!("\n\n=== EXACT ARDUINO WORKING SEQUENCE ===");

    // Step 1: EPD_ALL_Fill(WHITE) - Fill RAM with white pattern
    log::info!("Step 1: EPD_ALL_Fill(WHITE)");
    if let Err(e) = ssd1680.cpp_all_fill(Flag::AUTO_WRITE_PATTERN_ALL_WHITE) {
        log::error!("Failed to fill with white: {:?}", e);
    }
    delay.delay_ms(100);

    // Step 2: EPD_Update() - Trigger display update with 0xF4
    log::info!("Step 2: EPD_Update() - Trigger display refresh");
    if let Err(e) = ssd1680.cpp_update() {
        log::error!("Failed to update display: {:?}", e);
    }
    delay.delay_ms(100);

    // Step 3: EPD_Clear_R26H() - Clear R26h AFTER update (not before!)
    log::info!("Step 3: EPD_Clear_R26H() - Clear RED RAM after update");
    if let Err(e) = ssd1680.cpp_clear_r26h() {
        log::error!("Failed to clear R26h: {:?}", e);
    }

    log::info!("Arduino sequence complete. Display should show WHITE. Waiting 5 seconds...");
    delay.delay_ms(5000);

    // Now try with BLACK - this should make the screen turn black
    if let Err(e) = ssd1680.cpp_all_fill(Flag::AUTO_WRITE_PATTERN_ALL_BLACK) {
        log::error!("Failed to fill with black: {:?}", e);
    }
    delay.delay_ms(100);

    if let Err(e) = ssd1680.cpp_update() {
        log::error!("Failed to update display: {:?}", e);
    }
    delay.delay_ms(100);

    if let Err(e) = ssd1680.cpp_clear_r26h() {
        log::error!("Failed to clear R26h: {:?}", e);
    }

    log::info!("Display should be COMPLETELY BLACK");
    log::info!("Waiting 3 seconds...");
    delay.delay_ms(3000);

    // Clear to white first
    log::info!("Clearing display to white");
    if let Err(e) = ssd1680.cpp_all_fill(Flag::AUTO_WRITE_PATTERN_ALL_WHITE) {
        log::error!("Failed to fill white: {:?}", e);
    }
    if let Err(e) = ssd1680.cpp_update() {
        log::error!("Failed to update: {:?}", e);
    }
    if let Err(e) = ssd1680.cpp_clear_r26h() {
        log::error!("Failed to clear R26h: {:?}", e);
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
    // Write buffer to RAM (WRITE_BW_DATA)
    if let Err(e) = ssd1680.direct_cmd(Cmd::WRITE_BW_DATA) {
        log::error!("Failed to select RAM: {:?}", e);
        return Err(anyhow::anyhow!("Failed to select RAM: {:?}", e));
    }

    // Send the buffer data
    if let Err(e) = ssd1680.direct_data(display.buffer()) {
        log::error!("Failed to write buffer: {:?}", e);
        return Err(anyhow::anyhow!("Failed to write buffer: {:?}", e));
    }

    // Update display
    if let Err(e) = ssd1680.cpp_update() {
        log::error!("Failed to update display: {:?}", e);
        return Err(anyhow::anyhow!("Failed to update display: {:?}", e));
    }

    if let Err(e) = ssd1680.cpp_clear_r26h() {
        log::error!("Failed to clear R26h: {:?}", e);
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
        if let Err(e) = ssd1680.cpp_all_fill(Flag::AUTO_WRITE_PATTERN_ALL_WHITE) {
            log::error!("Failed to fill white: {:?}", e);
        }
        if let Err(e) = ssd1680.cpp_update() {
            log::error!("Failed to update: {:?}", e);
        }
        if let Err(e) = ssd1680.cpp_clear_r26h() {
            log::error!("Failed to clear R26h: {:?}", e);
        }
        delay.delay_ms(500);

        // Send image buffer to display
        log::info!("Sending logo buffer to display");
        if let Err(e) = ssd1680.direct_cmd(Cmd::WRITE_BW_DATA) {
            log::error!("Failed to select RAM: {:?}", e);
        } else {
            // Send the pre-converted image buffer
            if let Err(e) = ssd1680.direct_data(LOGO_IMAGE) {
                log::error!("Failed to write image buffer: {:?}", e);
            } else {
                // Update display
                if let Err(e) = ssd1680.cpp_update() {
                    log::error!("Failed to update display: {:?}", e);
                } else {
                    if let Err(e) = ssd1680.cpp_clear_r26h() {
                        log::error!("Failed to clear R26h: {:?}", e);
                    }

                    log::info!("LOGO DISPLAYED - CHECK SCREEN!");
                }
            }
        }

        delay.delay_ms(10000);
    }

    // Original test follows
    log::info!("\n\n=== ARDUINO CLEAR TO WHITE ===");
    if let Err(e) = ssd1680.direct_cmd(Cmd::BORDER_WAVEFORM_CONTROL) {
        log::error!("Failed to set border waveform: {:?}", e);
        return Err(anyhow::anyhow!("Failed to set border waveform: {:?}", e));
    }
    if let Err(e) = ssd1680.direct_data(&[Flag::BORDER_WAVEFORM_WHITE]) {
        log::error!("Failed to set border data: {:?}", e);
        return Err(anyhow::anyhow!("Failed to set border data: {:?}", e));
    }

    // Fill RAM with WHITE - trying 0x00 since display shows inverted (0xFF showed as black)
    log::info!("Filling RAM with WHITE (0x00) pixels - display polarity appears inverted");
    if let Err(e) = ssd1680.direct_cmd(Cmd::WRITE_BW_DATA) {
        log::error!("Failed to send write command: {:?}", e);
        return Err(anyhow::anyhow!("Failed to send write command: {:?}", e));
    }

    // Create a buffer to send in chunks more efficiently
    let white_buffer = vec![Flag::AUTO_WRITE_PATTERN_ALL_BLACK; 64]; // All WHITE (using black pattern - inverted polarity)

    for i in 0..(4736 / 64) {
        if i % 10 == 0 {
            log::info!("Sending chunk {}/{}", i, 4736 / 64);
        }

        if let Err(e) = ssd1680.direct_data(&white_buffer) {
            log::error!("Failed to write white data: {:?}", e);
            return Err(anyhow::anyhow!("Failed to write white data: {:?}", e));
        }
    }

    // Send any remaining bytes
    let remaining = 4736 % 64;
    if remaining > 0 {
        if let Err(e) = ssd1680.direct_data(&white_buffer[0..remaining]) {
            log::error!("Failed to write remaining white data: {:?}", e);
            return Err(anyhow::anyhow!(
                "Failed to write remaining white data: {:?}",
                e
            ));
        }
    }

    // Update display with proper sequence according to SSD1680 datasheet
    log::info!("Updating display with white - using proper update sequence");

    // Step 1: Specify which RAM to use for update - ONLY use BW RAM (0x24)
    if let Err(e) = ssd1680.direct_cmd(Cmd::DISPLAY_UPDATE_CTRL1) {
        log::error!("Failed to set display update control 1: {:?}", e);
        return Err(anyhow::anyhow!(
            "Failed to set display update control 1: {:?}",
            e
        ));
    }
    if let Err(e) = ssd1680.direct_data(&[Flag::DISPLAY_UPDATE_BW_RAM]) {
        // Use only BW RAM
        log::error!("Failed to set RAM usage: {:?}", e);
        return Err(anyhow::anyhow!("Failed to set RAM usage: {:?}", e));
    }

    // Step 2: Set update control with working C++ value
    if let Err(e) = ssd1680.direct_cmd(Cmd::DISPLAY_UPDATE_CTRL2) {
        log::error!("Failed to set display update control 2: {:?}", e);
        return Err(anyhow::anyhow!(
            "Failed to set display update control 2: {:?}",
            e
        ));
    }
    if let Err(e) = ssd1680.direct_data(&[Flag::DISPLAY_UPDATE_FULL]) {
        // Use full update sequence
        log::error!("Failed to set update control: {:?}", e);
        return Err(anyhow::anyhow!("Failed to set update control: {:?}", e));
    }

    // Step 3: Activate display update
    if let Err(e) = ssd1680.direct_cmd(Cmd::MASTER_ACTIVATE) {
        log::error!("Failed to activate display update: {:?}", e);
        return Err(anyhow::anyhow!(
            "Failed to activate display update: {:?}",
            e
        ));
    }

    // CRITICAL: Wait for BUSY pin to go LOW (display update takes ~2 seconds)
    log::info!("Waiting for display update to complete (BUSY pin)...");
    ssd1680.wait_busy();

    log::info!("Cleared to white. Waiting 3 seconds...");
    delay.delay_ms(3000);

    // Clear register 0x26 (RED RAM) with WHITE (0xFF) for proper operation
    log::info!("Clearing register 0x26 (RED RAM) with WHITE (0xFF)");
    if let Err(e) = ssd1680.direct_cmd(Cmd::WRITE_RED_DATA) {
        log::error!("Failed to select RAM: {:?}", e);
        return Err(anyhow::anyhow!("Failed to select RAM: {:?}", e));
    }

    // Create a simple test pattern (checkerboard)
    log::info!("\n\n=== ARDUINO TEST PATTERN ===");
    if let Err(e) = ssd1680.direct_cmd(Cmd::WRITE_BW_DATA) {
        log::error!("Failed to select RAM for pattern: {:?}", e);
        return Err(anyhow::anyhow!("Failed to select RAM for pattern: {:?}", e));
    }

    // Create a test pattern with multiple elements to help with diagnosis
    log::info!("Creating comprehensive test pattern buffer");
    let mut pattern_buffer = Vec::with_capacity(4736);

    // First 592 bytes (4 rows): Solid black
    for _ in 0..592 {
        pattern_buffer.push(Flag::AUTO_WRITE_PATTERN_ALL_BLACK);
    }

    // Next 592 bytes (4 rows): Solid white
    for _ in 0..592 {
        pattern_buffer.push(Flag::AUTO_WRITE_PATTERN_ALL_WHITE);
    }

    // Next 592 bytes (4 rows): Horizontal stripes
    for i in 0..592 {
        pattern_buffer.push(if (i / 16) % 2 == 0 {
            Flag::AUTO_WRITE_PATTERN_ALL_WHITE
        } else {
            Flag::AUTO_WRITE_PATTERN_ALL_BLACK
        });
    }

    // Next 592 bytes (4 rows): Vertical stripes
    for _ in 0..592 {
        pattern_buffer.push(Flag::AUTO_WRITE_PATTERN_CHECKERBOARD1); // 10101010 pattern
    }

    // Next 592 bytes (4 rows): Checkerboard
    for i in 0..592 {
        pattern_buffer.push(if (i / 16) % 2 == 0 {
            if i % 2 == 0 {
                Flag::AUTO_WRITE_PATTERN_CHECKERBOARD1
            } else {
                Flag::AUTO_WRITE_PATTERN_CHECKERBOARD2
            }
        } else if i % 2 == 0 {
            Flag::AUTO_WRITE_PATTERN_CHECKERBOARD2
        } else {
            Flag::AUTO_WRITE_PATTERN_CHECKERBOARD1
        });
    }

    // Fill remaining with border pattern
    for i in pattern_buffer.len()..4736 {
        // Create a border pattern - all black with white border
        let x = (i % 148) / 8; // X position in bytes (0-15)
        let y = i / 148; // Y position

        if x == 0 || x == 15 || !(4..=291).contains(&y) {
            pattern_buffer.push(Flag::AUTO_WRITE_PATTERN_ALL_WHITE); // White border
        } else {
            pattern_buffer.push(Flag::AUTO_WRITE_PATTERN_ALL_BLACK); // Black center
        }
    }

    // Send pattern in chunks to avoid watchdog timeouts
    log::info!("Sending checkerboard pattern in chunks");
    const CHUNK_SIZE: usize = 64; // Send 64 bytes at a time

    for (i, chunk) in pattern_buffer.chunks(CHUNK_SIZE).enumerate() {
        // Log progress occasionally
        if i % 10 == 0 {
            log::info!("Sending chunk {}/{}", i, pattern_buffer.len() / CHUNK_SIZE);
        }

        // Send this chunk
        if let Err(e) = ssd1680.direct_data(chunk) {
            log::error!("Failed to write pattern chunk: {:?}", e);
            return Err(anyhow::anyhow!("Failed to write pattern chunk: {:?}", e));
        }

        // Brief pause every 10 chunks to avoid watchdog timeouts
        if i % 10 == 9 {
            // Just a small yield to let the watchdog reset
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }
    }

    // Update with the pattern
    log::info!("Updating display with pattern");
    if let Err(e) = ssd1680.direct_update_display() {
        log::error!("Failed to update with pattern: {:?}", e);
        return Err(anyhow::anyhow!("Failed to update with pattern: {:?}", e));
    }

    log::info!("Test pattern displayed. Pausing 5 seconds...");
    delay.delay_ms(5000);

    // Success or failure summary
    log::info!("\n\n======================================================");
    log::info!("DISPLAY TEST SEQUENCE COMPLETED");
    log::info!("If the display is still blank after all these attempts:");
    log::info!("1. Check hardware connections (especially RST, DC, BUSY, SPI)");
    log::info!("2. Verify power supply to the e-paper display");
    log::info!("3. The display may be incompatible or damaged");
    log::info!("======================================================");
    delay.delay_ms(3000);

    // Now create text display if test pattern worked
    log::info!("Creating text display buffer");
    let mut display = Display2in13::new();
    display.set_rotation(DisplayRotation::Rotate90);

    // Clear text display buffer first
    display
        .clear(BinaryColor::Off)
        .expect("Failed to clear buffer");

    log::info!("Drawing text 1");
    // Try with inverted colors (Off=black on e-paper) in case display is inverting colors
    let style1 = MonoTextStyle::new(&ISO15_10, BinaryColor::Off);
    let text = Text::new("TEST PATTERN 1", Point::new(4, 30), style1);
    text.draw(&mut display).expect("Failed to draw text");

    log::info!("Drawing text 2");
    let style2 = MonoTextStyleBuilder::new()
        .font(&ISO15_10)
        .text_color(BinaryColor::Off) // Inverted color
        .build();
    Text::new("TEST PATTERN 2", Point::new(4, 60), style2)
        .draw(&mut display)
        .expect("Failed to draw text");

    log::info!("Drawing text 3");
    let style3 = MonoTextStyleBuilder::new()
        .font(&JIS_9)
        .text_color(BinaryColor::Off) // Inverted color
        .build();
    Text::new("TEST PATTERN 3", Point::new(4, 90), style3)
        .draw(&mut display)
        .expect("Failed to draw text");

    // Display updated frame
    // Add a functionality to test the display with a pattern if needed
    let test_pattern_fallback = true; // Change to true to try the test pattern instead

    if test_pattern_fallback {
        log::info!("Using test pattern instead of text display");

        // First clear the frame
        ssd1680
            .clear_frame(&mut delay)
            .expect("Failed to clear frame");

        // Then draw the test pattern
        log::info!("Drawing test pattern");
        ssd1680
            .draw_test_pattern(&mut delay)
            .expect("Failed to draw test pattern");

        log::info!("Test pattern should be visible on display");
    } else {
        log::info!("Update frame");
        ssd1680
            .update_frame(display.buffer())
            .expect("Failed to update black and white frame");

        log::info!("Display frame");

        // Use proper error handling with a panic message that includes the error
        if let Err(e) = ssd1680.display_frame(&mut delay) {
            log::error!("Failed to update display: {:?}", e);

            // Try the test pattern as a fallback
            log::info!("Attempting fallback to test pattern...");
            if let Err(e2) = ssd1680.draw_test_pattern(&mut delay) {
                log::error!("Test pattern also failed: {:?}", e2);
                panic!("Display update failed with all methods");
            } else {
                log::info!("Test pattern displayed successfully as fallback");
            }
        } else {
            log::info!("Display frame updated successfully");
        }
    }

    // Add a pause to let the display stabilize
    log::info!("Pausing to allow display stabilization");
    delay.delay_ms(1000);

    log::info!("Display update complete!");

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

// External buttons and their GPIO pin numbers
const BTN_EXIT: u8 = 1;
const BTN_MENU: u8 = 2;
const BTN_UP: u8 = 6;
const BTN_DOWN: u8 = 4;
const BTN_CONF: u8 = 5;
const BTN_RESET: u8 = 3;

// Other useful pins
const PIN_POWER_LED: u8 = 41;

// TF card pins
const TFC_CS: u8 = 10;
const TFC_MOSI: u8 = 40;
const TFC_MISO: u8 = 13;
const TFC_CLK: u8 = 39;
/*
// Go look at
// https://github.com/esp-rs/esp-idf-svc/blob/master/examples/sd_spi.rs
fn connect_to_sdcard((peripherals: &Peripherals) -> ! {
    let cs = peripherals.pins.gpio10;
}
*/

fn draw_rotation_and_rulers(display: &mut Display2in13) {
    display.set_rotation(DisplayRotation::Rotate0);
    draw_text(display, "rotation 0", 50, 35);
    draw_ruler(display);

    display.set_rotation(DisplayRotation::Rotate90);
    draw_text(display, "rotation 90", 50, 35);
    draw_ruler(display);

    display.set_rotation(DisplayRotation::Rotate180);
    draw_text(display, "rotation 180", 50, 35);
    draw_ruler(display);

    display.set_rotation(DisplayRotation::Rotate270);
    draw_text(display, "rotation 270", 50, 35);
    draw_ruler(display);
}

fn draw_ruler(display: &mut Display2in13) {
    for col in 1..ssd1680::WIDTH {
        if col % 25 == 0 {
            Line::new(Point::new(col as i32, 0), Point::new(col as i32, 10))
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)
                .unwrap();
        }

        if col % 50 == 0 {
            let label = col.to_string();
            draw_text(display, &label, col as i32, 20);
        }
    }
}

fn draw_text(display: &mut Display2in13, text: &str, x: i32, y: i32) {
    let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::Off);
    let _ = Text::with_text_style(
        text,
        Point::new(x, y),
        style,
        TextStyleBuilder::new().alignment(Alignment::Center).build(),
    )
    .draw(display);
}
