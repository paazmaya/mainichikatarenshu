use anyhow::Ok;

use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Line;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::text::Alignment;
use embedded_graphics::text::TextStyleBuilder;
use embedded_graphics::{prelude::*, text::Text};
use ssd1680::color::Black;
use ssd1680::prelude::*;
// https://docs.rs/embedded-graphics/0.8.1/embedded_graphics/mono_font/index.html#modules
use embedded_graphics::mono_font::{
    iso_8859_15::FONT_10X20 as ISO15_10, jis_x0201::FONT_9X15 as JIS_9, MonoTextStyle,
    MonoTextStyleBuilder,
};

use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::peripherals::Peripherals;

use esp_idf_svc::hal::gpio;
use esp_idf_svc::hal::peripheral;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::spi;

// https://docs.esp-rs.org/esp-idf-svc/esp_idf_svc/
fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    let peripherals = Peripherals::take().expect("Could not take peripherals");
    let pins = peripherals.pins;

    // Pins on the ESP32S3 that are connected to E-paper display
    waveshare_epd_hello_world(
        peripherals.spi2,
        pins.gpio12.into(),
        pins.gpio11.into(),
        pins.gpio45.into(),
        pins.gpio48.into(),
        pins.gpio46.into(),
        pins.gpio47.into(),
    )
    .expect("Failed to initialize Waveshare e-paper display");

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

fn waveshare_epd_hello_world(
    spi: impl peripheral::Peripheral<P = impl spi::SpiAnyPins> + 'static,
    sclk: gpio::AnyOutputPin,
    sdo: gpio::AnyOutputPin,
    cs: gpio::AnyOutputPin,
    busy_in: gpio::AnyInputPin,
    dc: gpio::AnyOutputPin,
    rst: gpio::AnyOutputPin,
) -> anyhow::Result<()> {
    log::info!("About to initialize Waveshare e-paper display");

    let mut driver = spi::SpiDeviceDriver::new_single(
        spi,
        sclk,
        sdo,
        Option::<gpio::AnyIOPin>::None,
        Some(cs),
        &spi::SpiDriverConfig::new().dma(spi::Dma::Disabled),
        &spi::SpiConfig::new().baudrate(26.MHz().into()),
    )
    .expect("Could not create SPI device driver");
    let mut delay = Delay::default();

    let mut ssd1680 = Ssd1680::new(
        &mut driver,
        gpio::PinDriver::input(busy_in).expect("Failed to set busy pin as input"),
        gpio::PinDriver::output(dc).expect("Failed to set dc pin as output"),
        gpio::PinDriver::output(rst).expect("Failed to set rst pin as output"),
        &mut delay,
    )
    .expect("Could not create EPD driver");
    ssd1680.init(&mut delay).expect("Failed to initialize display");

    // Clear frames on the display driver
    ssd1680
        .clear_red_frame()
        .expect("Failed to clear red frame");
    ssd1680
        .clear_bw_frame()
        .expect("Failed to clear black and white frame");

    // Create buffer for black and white
    let mut display = Display2in13::bw();

    display
        .clear(BinaryColor::Off)
        .expect("Failed to clear display");

    draw_rotation_and_rulers(&mut display);

    // Two ways to create style
    let style1 = MonoTextStyle::new(&ISO15_10, BinaryColor::On);
    let style2 = MonoTextStyleBuilder::new()
        .font(&ISO15_10)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();
    let text = Text::new("Hei senkin tonttu, yritä nyt...", Point::new(4, 10), style1);
    text.draw(&mut display).expect("Failed to draw text");

    // Create a text at position (20, 30) and draw it using the previously defined style
    Text::new("...saada tämä toiminmanhan", Point::new(4, 36), style2)
        .draw(&mut display)
        .expect("Failed to draw text");

    let style3 = MonoTextStyleBuilder::new()
        .font(&JIS_9)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();
    Text::new(
        // Japanese text katakana only
        "カタカナ",
        Point::new(4, 60),
        style3,
    )
    .draw(&mut display)
    .expect("Failed to katakana draw text");

    // Display updated frame
    log::info!("Send bw frame to display");
    ssd1680
        .update_bw_frame(&display.buffer())
        .expect("Failed to update black and white frame");
    ssd1680
        .display_frame(&mut delay)
        .expect("Failed to display frame");

    Ok(())
}

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
                .into_styled(PrimitiveStyle::with_stroke(Black, 1))
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
