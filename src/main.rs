use core::time::Duration;
use std::{thread, time};

use anyhow::{Error, Ok};

use epd_waveshare::{
    color::{self, White},
    epd2in9_v2::{Display2in9, Epd2in9, DEFAULT_BACKGROUND_COLOR, HEIGHT, WIDTH},
    graphics::DisplayRotation,
    prelude::*,
};

use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    prelude::*,
    primitives::{Circle, PrimitiveStyleBuilder},
    text::{Baseline, Text, TextStyleBuilder},
};
use embedded_graphics::{
    pixelcolor::BinaryColor::On as Black,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};

use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::*;

use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::gpio::{IOPin, InputPin, OutputPin};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::spi::SpiDeviceDriver;
use esp_idf_svc::hal::{
    delay::Delay,
    gpio::{AnyIOPin, Pin},
    spi::{config::DriverConfig, Dma, SpiDriver},
};

use esp_idf_svc::sys::EspError;

use esp_idf_svc::hal::adc;
use esp_idf_svc::hal::delay;
use esp_idf_svc::hal::gpio;
use esp_idf_svc::hal::i2c;
use esp_idf_svc::hal::peripheral;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::spi;

use esp_idf_svc::eventloop::*;
use esp_idf_svc::ipv4;
use esp_idf_svc::mqtt::client::*;
use esp_idf_svc::ping;
use esp_idf_svc::sntp;
use esp_idf_svc::systime::EspSystemTime;
use esp_idf_svc::timer::*;
use esp_idf_svc::wifi::*;

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

    let mut delay = Delay::default();

    // Pins on the ESP32S3 that are connected to E-paper
    /*
        let sclk = peripherals.pins.gpio12.downgrade();
        let cs = peripherals.pins.gpio45.into();
        let mosi = peripherals.pins.gpio11.downgrade_output();

        let busy = PinDriver::input(peripherals.pins.gpio48).expect("Could not set pin as input");
        let dc = PinDriver::output(peripherals.pins.gpio46).expect("Could not set pin as input");
        let rst = PinDriver::output(peripherals.pins.gpio47).expect("Could not set pin as input");
    */
    /*
    let dma = Dma::Auto(4096);
    let spi = SpiDriver::new(
        peripherals.spi2,
        sclk, // Pass the sclk pin directly
        mosi,
        AnyIOPin::none(),
        &DriverConfig::default().dma(dma),
    )?;
    */

    waveshare_epd_hello_world(
        peripherals.spi2,
        pins.gpio12.into(),
        pins.gpio11.into(),
        pins.gpio45.into(),
        pins.gpio48.into(),
        pins.gpio46.into(),
        pins.gpio47.into(),
    )?;

    /*
        let mut spi_device = SpiDeviceDriver::new(spi, cs, &Default::default())?;

        let mut epd = Epd2in9::new(&mut spi_device, busy, dc, rst, &mut delay, None)?;
        log::info!("epd setup completed");

        let mut display = Display2in9::default();
        display.clear(color::Color::White).expect("Could not clear display");
    */
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

/// Retuns the size of a buffer necessary to hold the entire image
pub fn get_buffer_size() -> usize {
    // The height is multiplied by 2 because the red pixels essentially exist on a separate "layer"
    epd_waveshare::buffer_len(WIDTH as usize, HEIGHT as usize * 2)
}

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
        Option::<gpio::AnyOutputPin>::None,
        &spi::SpiDriverConfig::new().dma(spi::Dma::Disabled),
        &spi::SpiConfig::new().baudrate(26.MHz().into()),
    )
    .expect("Could not create SPI device driver");
    let mut delay = Delay::default();

    // Setup EPD
    let mut epd = Epd2in9::new(
        &mut driver,
        gpio::PinDriver::output(cs)?,
        gpio::PinDriver::input(busy_in)?,
        gpio::PinDriver::output(dc)?,
        gpio::PinDriver::output(rst)?,
        &mut delay,
    )
    .expect("Could not create EPD driver");

    // Two ways of making the display
    let mut display = Display2in9::default();

    // Use display graphics from embedded-graphics
    //let mut buffer = vec![DEFAULT_BACKGROUND_COLOR.get_byte_value(); WIDTH as usize / 8 * HEIGHT as usize];
    //let mut display = VarDisplay::new(WIDTH, HEIGHT, &mut buffer);
    display.set_rotation(DisplayRotation::Rotate90);
    display.clear_buffer(DEFAULT_BACKGROUND_COLOR);

    // Write "Hello, world!" to the screen
    //epd.clear_frame(&mut driver, &mut delay);

    // Two ways to crate style
    let style1 = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    let style2 = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();
    let text = Text::new("Hello, world!", Point::new(10, 10), style1);
    //text.draw(&mut display)?;

    // Create a text at position (20, 30) and draw it using the previously defined style
    //Text::new("Hello Rust!", Point::new(20, 30), style1).draw(&mut display)?;

    // Display updated frame
    epd.update_frame(&mut driver, &display.buffer(), &mut delay)?;
    epd.display_frame(&mut driver, &mut delay)?;

    Ok(())
}
