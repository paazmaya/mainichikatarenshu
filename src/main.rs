#![no_std]
#![no_main]

use esp_alloc as _;
use esp_backtrace as _;
use esp_println::println;

use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{AnyInputPin, Level, OutputOpenDrain, PinDriver, Pull, *},
    i2c::master::{Config, I2c},
    init,
    modem::Modem,
    nvs::EspDefaultNvsPartition,
    peripherals::Peripherals,
    prelude::*,
    sleep::{self, Duration},
    spi::{self, Spi, SpiBus, SpiDevice},
};

use defmt::Format;

use epd_waveshare::prelude::*;
use epd_waveshare::{
    color::*,
    epd2in9::{Epd2in9, HEIGHT, WIDTH},
    graphics::VarDisplay,
};

type SpiDev = SpiDeviceDriver<'static, SpiDriver<'static>>;

type EpdDriver = Epd2in9<
    SpiDev,
    PinDriver<'static, AnyOutputPin, Output>,
    PinDriver<'static, AnyInputPin, Input>,
    PinDriver<'static, AnyOutputPin, Output>,
    PinDriver<'static, AnyOutputPin, Output>,
    Delay,
>;

#[derive(Format, Debug)]
pub enum MyError {
    GpioError,
    SpiError,
    OtherError,
}

#[entry]
fn main() -> ! {
    // Initialize with the highest possible frequency for this chip
    // https://docs.esp-rs.org/esp-hal/esp-hal/0.22.0/esp32c3/esp_hal/peripherals/index.html
    let peripherals = init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    esp_alloc::heap_allocator!(72 * 1024);

    let mut delay = Delay::new();

    let spi = Spi::new(peripherals.SPI1, 100.kHz(), spi::SpiMode::Mode0);

    let i2c = I2c::new(peripherals.I2S0, Config::default());

    println!("Hello world!");
    delay.delay(500.millis());

    // Use "exit" button to wake up
    let wakeup_pin: AnyInputPin = peripherals.GPIO1.into_input().into();
    let sleep_time = Duration::from_micros(5_000_000);
    enter_deep_sleep(wakeup_pin, sleep_time);
}

fn youdoit() -> Result<(), MyError> {
    // Bind the log crate to the ESP Logging facilities
    println!("wakeup: {:?}", esp_hal::reset::WakeupReason::get());

    let peripherals = init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });
    let (_led_pin, wakeup_pin, modem, spi_driver, epd, delay) = gather_peripherals(peripherals)?;

    // deep sleep for 1.5 hours (or on wakeup button press)
    enter_deep_sleep(wakeup_pin.into(), Duration::from_secs(60 * 30 * 3));

    unreachable!("in sleep");
}

fn gather_peripherals(
    peripherals: Peripherals,
) -> Result<(Gpio2, Gpio4, Modem, SpiDev, EpdDriver, Delay), MyError> {
    let ledpin = peripherals.GPIO2;
    let wakeup_pin = peripherals.GPIO4;

    let modem = peripherals.modem;

    let spi_p = peripherals.spi3;
    let sclk: AnyOutputPin = peripherals.GPIO13.into();
    let sdo: AnyOutputPin = peripherals.GPIO14.into();
    let cs: AnyOutputPin = peripherals.GPIO15.into();
    let busy_in: AnyInputPin = peripherals.GPIO25.into();
    let rst: AnyOutputPin = peripherals.GPIO26.into();
    let dc: AnyOutputPin = peripherals.GPIO27.into();

    println!("create epd driver");
    let (spi_driver, epd, delay) = create_epd_driver(spi_p, sclk, sdo, cs, busy_in, rst, dc)?;

    Ok((ledpin, wakeup_pin, modem, spi_driver, epd, delay))
}

fn enter_deep_sleep(wakeup_pin: AnyInputPin, sleep_time: Duration) {
    // Configure the wakeup pin as an input
    let wakeup_pin = PinDriver::input(wakeup_pin).expect("Failed to configure wakeup pin");

    // Enable external wakeup on the specified pin
    sleep::enable_ext0_wakeup(wakeup_pin.pin(), sleep::WakeupLevel::Low)
        .expect("Failed to enable ext0 wakeup");

    // Log the deep sleep entry
    println!("Entering deep sleep");

    // Enter deep sleep
    sleep::deep_sleep(sleep_time);

    // The program will not reach this point because the device will be in deep sleep
    unreachable!("We will be asleep by now");
}

fn create_epd_driver(
    spi_p: spi::SPI3,
    sclk: AnyOutputPin,
    sdo: AnyOutputPin,
    cs: AnyOutputPin,
    busy_in: AnyInputPin,
    rst: AnyOutputPin,
    dc: AnyOutputPin,
) -> Result<(SpiDev, EpdDriver, Delay), MyError> {
    let mut driver = spi::SpiDeviceDriver::new_single(
        spi_p,
        sclk,
        sdo,
        Option::<gpio::AnyIOPin>::None,
        Option::<gpio::AnyOutputPin>::None,
        &spi::config::DriverConfig::new(),
        &spi::config::Config::new().baudrate(10.MHz().into()),
    )?;

    println!("driver setup completed");
    let mut delay = Delay {};

    // Setup EPD
    let epd_driver = Epd7in5::new(
        &mut driver,
        PinDriver::output(cs)?,
        PinDriver::input(busy_in)?,
        PinDriver::output(dc)?,
        PinDriver::output(rst)?,
        &mut delay,
    )
    .unwrap();

    println!("epd setup completed");

    Ok((driver, epd_driver, delay))
}

/// Retuns the size of a buffer necessary to hold the entire image
pub fn get_buffer_size() -> usize {
    // The height is multiplied by 2 because the red pixels essentially exist on a separate "layer"
    epd_waveshare::buffer_len(WIDTH as usize, HEIGHT as usize * 2)
}
