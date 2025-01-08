use core::time::Duration;


use epd_waveshare::{color::*, graphics::VarDisplay};
use epd_waveshare::{epd2in9::*, prelude::*};

use embedded_graphics::{
    pixelcolor::BinaryColor::On as Black,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};




fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");
}





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


/*
fn create_epd_driver(peripherals: &Peripherals, delay: &Delay) -> Result<(EpdDriver), MyError> {

    // SCLK (Clock) – Line for the clock signal.
    let sclk = peripherals.GPIO12.degrade();
    // SS/CS (Slave Select/Chip Select) – Line for the master to select which slave to send data to.
    let cs:AnyPin = peripherals.GPIO45.degrade();
    // MOSI (Master Output/Slave Input) – Line for the master to send data to the slave.
    let mosi:AnyPin = peripherals.GPIO11.degrade();

    // 10.MHz()
    let mut spi = Spi::new(
        peripherals.SPI2,
    )
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_cs(cs);

    println!("driver setup completed");

    // SPI: SpiDevice,
    // BUSY: InputPin, An output from the display, if LOW then the display is busy and cannot accept data.
    let busy = peripherals.GPIO48.degrade();
    // DC: OutputPin, Data/Command input. If held HIGH then the SPI bus is sending data, if LOW then it is sending command signals.
    let dc = OutputPin::new(peripherals.GPIO46, Level::Low, OutputOpenDrain, Pull::Up).expect("Failed to configure DC pin");
    // RST: OutputPin, External Reset, send this pin LOW to reset the display.
    let rst = OutputPin::new(peripherals.GPIO47, Level::Low, OutputOpenDrain, Pull::Up).expect("Failed to configure RST pin");

    let mut epd = Epd2in9::new(&mut spi, busy, dc, rst, &mut delay, None)?;

    // Use display graphics from embedded-graphics
    let mut display = Display2in9::default();

    println!("epd setup completed");

    Ok(epd_driver)
}

/// Retuns the size of a buffer necessary to hold the entire image
pub fn get_buffer_size() -> usize {
    // The height is multiplied by 2 because the red pixels essentially exist on a separate "layer"
    epd_waveshare::buffer_len(WIDTH as usize, HEIGHT as usize * 2)
}
*/
