#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use chrono::{DateTime, NaiveDate, Timelike, Utc};
use core::panic::PanicInfo;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_futures::block_on;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    rng::Rng,
    rtc_cntl::{reset_reason, Rtc, SocResetReason},
    spi::master::{Config, Spi},
    system::Cpu,
    time::Rate,
    uart::{Config as UartConfig, Uart},
};
use esp_storage::FlashStorage;
use heapless::String;
use mainichikatarenshu::{
    buttons::{Debouncer, Event},
    kata::KATA,
    kata_display::KataDisplayManager,
    partitions::training_partition,
    ssd1680::{
        display_utils::DisplayManager,
        driver::Ssd1680,
        graphics::{Display, Display2in13},
    },
    storage::{PersistentTraining, StateStore, TransactionError},
    training::{Change, Error as TrainingError},
};

#[cfg(feature = "wifi")]
mod wireless;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    esp_println::println!("Firmware error: {info}");
    loop {}
}

#[cfg(not(feature = "wifi"))]
#[esp_hal::main]
fn main() -> ! {
    block_on(application())
}

#[cfg(feature = "wifi")]
#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    application(spawner).await
}

async fn application(#[cfg(feature = "wifi")] spawner: embassy_executor::Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    #[cfg(not(feature = "wifi"))]
    esp_alloc::heap_allocator!(size: 64 * 1024);
    #[cfg(feature = "wifi")]
    esp_alloc::heap_allocator!(size: 128 * 1024);

    #[cfg(feature = "wifi")]
    let wifi_available = {
        let timer = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
        esp_rtos::start(timer.timer0, peripherals.FROM_CPU_INTR0);
        match wireless::wifi_task(peripherals.WIFI, spawner) {
            Ok(task) => {
                spawner.spawn(task);
                true
            }
            Err(error) => {
                esp_println::println!("Wi-Fi task failed to start: {error:?}");
                false
            }
        }
    };

    let spi = Spi::new(
        peripherals.SPI2,
        Config::default().with_frequency(Rate::from_mhz(4)),
    )
    .expect("SPI configuration failed")
    .with_sck(peripherals.GPIO12)
    .with_mosi(peripherals.GPIO11);
    let cs = Output::new(peripherals.GPIO45, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO46, Level::Low, OutputConfig::default());
    let rst = Output::new(peripherals.GPIO47, Level::High, OutputConfig::default());
    let busy = Input::new(peripherals.GPIO48, InputConfig::default());
    let spi = ExclusiveDevice::new(spi, cs, Delay::new()).expect("SPI CS initialization failed");
    let mut panel =
        Ssd1680::new(spi, busy, dc, rst, Delay::new()).expect("Display initialization failed");
    let mut display = Display2in13::new();

    if KATA.is_empty() {
        DisplayManager::show_centered_message(&mut display, "kata.yaml missing", None)
            .expect("Display rendering failed");
        panel
            .write_buffer_and_update(display.buffer())
            .expect("Display update failed");
        panic!("Configure kata.yaml and rebuild the firmware");
    }

    let mut flash = FlashStorage::new(peripherals.FLASH);
    let mut scratch = [0u8; 4096];
    flash
        .read(0x8000, &mut scratch)
        .expect("Cannot read partition table");
    let range = training_partition(&scratch, flash.capacity() as u32)
        .expect("Invalid or missing training partition");
    let store =
        StateStore::new(BlockingAsync::new(flash), range).expect("Invalid training storage range");
    let mut training =
        block_on(PersistentTraining::load(store, &mut scratch)).expect("Cannot load training data");

    let mut uart = Uart::new(peripherals.UART0, UartConfig::default())
        .expect("UART configuration failed")
        .with_rx(peripherals.GPIO44)
        .with_tx(peripherals.GPIO43);
    let rtc = Rtc::new(peripherals.RTC_TIMER);
    let mut time_valid = reset_reason(Cpu::ProCpu) == Some(SocResetReason::CoreDeepSleep)
        && rtc.current_time_us() / 1_000_000 >= 946_684_800;
    let input = Input::new(
        peripherals.GPIO5,
        InputConfig::default().with_pull(Pull::Up),
    );
    let mut button = Debouncer::new(50, rtc.time_since_power_up().as_millis());
    let mut command: String<96> = String::new();
    #[cfg(feature = "wifi")]
    let mut wifi_ssid: String<32> = String::new();
    #[cfg(feature = "wifi")]
    let mut wifi_password: String<63> = String::new();
    #[cfg(feature = "wifi")]
    let mut wifi_submitted = false;
    let mut overflow = false;
    let mut last_second = None;
    let mut training_error = None;
    let mut redraw = true;
    let rng = Rng::new();
    #[cfg(not(feature = "wifi"))]
    let delay = Delay::new();

    esp_println::println!("Commands: TIME <UTC unix seconds>, LIST, ACK <YYYY-MM-DD>");
    #[cfg(feature = "wifi")]
    esp_println::println!("Wi-Fi: SSID <name>, PASSWORD <passphrase>, CONNECT");
    loop {
        while uart.read_ready() {
            let mut byte = [0u8; 1];
            uart.read(&mut byte).expect("UART read failed");
            match byte[0] {
                b'\r' | b'\n' if overflow => {
                    esp_println::println!("Command too long");
                    command.clear();
                    overflow = false;
                }
                b'\r' | b'\n' if !command.is_empty() => {
                    if let Some(value) = command.strip_prefix("TIME ") {
                        match value.parse::<u64>().ok().and_then(|seconds| {
                            let micros = seconds.checked_mul(1_000_000)?;
                            DateTime::<Utc>::from_timestamp(i64::try_from(seconds).ok()?, 0)?;
                            Some(micros)
                        }) {
                            Some(micros) if micros >= 946_684_800_000_000 => {
                                rtc.set_current_time_us(micros);
                                time_valid = true;
                                last_second = None;
                                redraw = true;
                                esp_println::println!("Clock set");
                            }
                            _ => esp_println::println!(
                                "Invalid TIME; provide UTC unix seconds since 2000"
                            ),
                        }
                    } else if command == "LIST" {
                        esp_println::println!(
                            "Pending records: {}",
                            training.state().pending().len()
                        );
                        for record in training.state().pending() {
                            esp_println::println!(
                                "{} | {:?} | confirmed: {:?}",
                                record.date,
                                record.kata,
                                record.confirmed_at
                            );
                        }
                    } else if let Some(value) = command.strip_prefix("ACK ") {
                        match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
                            Ok(date) => match block_on(training.acknowledge(&mut scratch, date)) {
                                Ok(_) => {
                                    esp_println::println!("Record acknowledged");
                                    last_second = None;
                                }
                                Err(TransactionError::Training(error)) => {
                                    esp_println::println!("ACK failed: {error:?}")
                                }
                                Err(TransactionError::Storage(error)) => {
                                    panic!("ACK storage failed: {error:?}")
                                }
                                Err(TransactionError::RequiresReload) => {
                                    panic!("Training storage requires reload")
                                }
                            },
                            Err(_) => esp_println::println!("Invalid ACK date"),
                        }
                    } else if cfg!(feature = "wifi") && command.starts_with("SSID ") {
                        #[cfg(feature = "wifi")]
                        {
                            match String::try_from(&command[5..]) {
                                Ok(value) => {
                                    wifi_ssid = value;
                                    esp_println::println!("SSID received");
                                }
                                Err(_) => {
                                    wifi_ssid.clear();
                                    esp_println::println!("SSID too long");
                                }
                            }
                        }
                    } else if cfg!(feature = "wifi") && command.starts_with("PASSWORD ") {
                        #[cfg(feature = "wifi")]
                        {
                            match String::try_from(&command[9..]) {
                                Ok(value) => {
                                    wifi_password = value;
                                    esp_println::println!("Password received");
                                }
                                Err(_) => {
                                    wifi_password.clear();
                                    esp_println::println!("Password too long");
                                }
                            }
                        }
                    } else if cfg!(feature = "wifi") && command == "CONNECT" {
                        #[cfg(feature = "wifi")]
                        {
                            if !wifi_available {
                                esp_println::println!("Wi-Fi task unavailable; restart to retry");
                            } else if wifi_submitted {
                                esp_println::println!("Restart to change Wi-Fi credentials");
                            } else {
                                match mainichikatarenshu::network_config::Credentials::new(
                                    &wifi_ssid,
                                    &wifi_password,
                                ) {
                                    Ok(credentials) => match wireless::CREDENTIALS
                                        .try_send(credentials)
                                    {
                                        Ok(()) => {
                                            wifi_submitted = true;
                                            wifi_password.clear();
                                            esp_println::println!("Wi-Fi connection requested");
                                        }
                                        Err(_) => esp_println::println!("Wi-Fi request queue full"),
                                    },
                                    Err(error) => esp_println::println!(
                                        "Invalid Wi-Fi configuration: {error:?}"
                                    ),
                                }
                            }
                        }
                    } else {
                        esp_println::println!("Unknown command");
                    }
                    command.clear();
                }
                b'\r' | b'\n' => {}
                byte if !overflow && byte.is_ascii() => {
                    if command.push(char::from(byte)).is_err() {
                        overflow = true;
                    }
                }
                _ => overflow = true,
            }
        }

        let millis = rtc.time_since_power_up().as_millis();
        let event = button
            .sample(input.is_high(), millis)
            .expect("Monotonic clock moved backwards");
        if time_valid {
            let seconds = rtc.current_time_us() / 1_000_000;
            let now = DateTime::<Utc>::from_timestamp(
                i64::try_from(seconds).expect("RTC timestamp overflow"),
                0,
            )
            .expect("RTC timestamp out of range");
            if last_second != Some(seconds) {
                let count = u32::try_from(KATA.len()).expect("Too many kata");
                let limit = u32::MAX - u32::MAX % count;
                let selected = loop {
                    let value = rng.random();
                    if value < limit {
                        break KATA[(value % count) as usize];
                    }
                };
                match block_on(training.advance(&mut scratch, now, Some(selected))) {
                    Ok(change) => {
                        if training_error.take().is_some() || change != Change::None {
                            redraw = true;
                        }
                    }
                    Err(TransactionError::Training(error)) => {
                        if training_error != Some(error) {
                            esp_println::println!("Cannot advance training: {error:?}");
                            training_error = Some(error);
                            redraw = true;
                        }
                    }
                    Err(TransactionError::Storage(error)) => {
                        panic!("Training storage failed: {error:?}")
                    }
                    Err(TransactionError::RequiresReload) => {
                        panic!("Training storage requires reload")
                    }
                }
                last_second = Some(seconds);
            }
            if event == Some(Event::Pressed) && training_error.is_none() {
                match block_on(training.confirm(&mut scratch, now)) {
                    Ok(Change::Confirmed) => redraw = true,
                    Ok(_) => {}
                    Err(TransactionError::Training(error)) => {
                        esp_println::println!("Confirmation ignored: {error:?}")
                    }
                    Err(TransactionError::Storage(error)) => {
                        panic!("Confirmation storage failed: {error:?}")
                    }
                    Err(TransactionError::RequiresReload) => {
                        panic!("Training storage requires reload")
                    }
                }
            }
            if redraw {
                DisplayManager::clear_and_prepare(&mut display).expect("Display clear failed");
                if let Some(error) = training_error {
                    let message = match error {
                        TrainingError::QueueFull => "Queue full: LIST / ACK",
                        TrainingError::ClockWentBackwards => "Clock moved backwards",
                        _ => "Training error: see serial",
                    };
                    DisplayManager::show_centered_message(&mut display, message, None)
                        .expect("Display rendering failed");
                } else if let Some(record) = training.state().active() {
                    let local = now.with_timezone(&chrono_tz::Europe::Helsinki);
                    let status = if record.confirmed_at.is_some() {
                        "Completed"
                    } else {
                        "Press confirm"
                    };
                    KataDisplayManager::show_kata_reminder(
                        &mut display,
                        &format!("{}", local.date_naive()),
                        &format!("{:02}:{:02}", local.hour(), local.minute()),
                        &record.kata,
                        Some(status),
                    )
                    .expect("Display rendering failed");
                } else {
                    DisplayManager::show_centered_message(&mut display, "Rest until 07:00", None)
                        .expect("Display rendering failed");
                }
                panel
                    .write_buffer_and_update(display.buffer())
                    .expect("Display update failed");
                redraw = false;
            }
        } else if redraw {
            DisplayManager::clear_and_prepare(&mut display).expect("Display clear failed");
            DisplayManager::show_centered_message(&mut display, "Set time via serial", None)
                .expect("Display rendering failed");
            panel
                .write_buffer_and_update(display.buffer())
                .expect("Display update failed");
            redraw = false;
        }
        #[cfg(not(feature = "wifi"))]
        delay.delay_millis(25);
        #[cfg(feature = "wifi")]
        embassy_time::Timer::after_millis(25).await;
    }
}
