//! Firmware entry point for the CrowPanel ESP32-S3 2.9" e-paper display.
//!
//! This binary only wires peripherals to the hardware-independent logic in
//! the library crate: it polls the serial console, buttons and (with the
//! `wifi` feature) network channels, advances the training state and redraws
//! the panel when something visible changed.

#![no_std]
#![no_main]

extern crate alloc;

use chrono::{DateTime, NaiveDate, Utc};
use core::panic::PanicInfo;
use crowpanel_ssd1680::{Display, Display2in9, Ssd1680};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_futures::block_on;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull},
    peripherals::{FLASH, SPI2, UART0},
    rng::Rng,
    rtc_cntl::{reset_reason, Rtc, SocResetReason},
    spi::master::{Config as SpiConfig, Spi},
    system::Cpu,
    time::Rate,
    uart::{Config as UartConfig, Uart},
    Blocking,
};
use esp_storage::FlashStorage;
use heapless::String;
use mainichikatarenshu::{
    buttons::{ButtonBank, ButtonId},
    clock,
    console::{parse_command, Command, CommandError, LineBuffer, LineError},
    kata::KATA,
    layout,
    partitions::find_training_partition,
    schedule::seconds_until_next_transition,
    screens::{draw_screen, select_screen},
    storage::{PersistentTraining, StateStore, TransactionError},
    training::{choose_kata, Change, Error as TrainingError},
};

#[cfg(feature = "wifi")]
mod wireless;

// The ESP-IDF 2nd stage bootloader (provided by `espflash`) requires this descriptor.
esp_bootloader_esp_idf::esp_app_desc!();

/// Offset of the ESP partition table in flash.
const PARTITION_TABLE_OFFSET: u32 = 0x8000;
/// Button debounce interval in milliseconds.
const DEBOUNCE_MS: u64 = 50;
/// Time for the panel supply to stabilize after enabling it, in milliseconds.
const EPD_POWER_SETTLE_MS: u32 = 100;
/// Main loop polling interval in milliseconds.
const POLL_INTERVAL_MS: u32 = 25;
/// Longest single rest pause, so serial input is still noticed regularly.
const MAX_REST_SECS: u64 = 60;

type Panel = Ssd1680<
    ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>,
    Input<'static>,
    Output<'static>,
    Output<'static>,
    Delay,
>;
type Training = PersistentTraining<BlockingAsync<FlashStorage<'static>>>;
type TrainingResult<T> = Result<T, TransactionError<esp_storage::FlashStorageError>>;

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
    esp_alloc::heap_allocator!(size: 160 * 1024);

    #[cfg(feature = "wifi")]
    let wifi_available = {
        let timer = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
        esp_rtos::start(timer.timer0, peripherals.FROM_CPU_INTR0);
        wireless::start(peripherals.WIFI, peripherals.RNG, peripherals.ADC1, spawner)
    };

    // The panel is powered through GPIO7 (`Pins::EPD_POWER`, `LCD_3.3_CTL`). Without it the
    // controller never answers; keep the pin driven for as long as the firmware runs.
    let _epd_power = Output::new(peripherals.GPIO7, Level::High, OutputConfig::default());
    Delay::new().delay_millis(EPD_POWER_SETTLE_MS);

    let mut panel = init_panel(
        peripherals.SPI2,
        peripherals.GPIO12,
        peripherals.GPIO11,
        peripherals.GPIO45,
        peripherals.GPIO46,
        peripherals.GPIO47,
        peripherals.GPIO48,
    );
    let mut display = Display2in9::new();
    if KATA.is_empty() {
        halt_unconfigured(&mut panel, &mut display);
    }

    let mut scratch = [0u8; 4096];
    let training = load_training(peripherals.FLASH, &mut scratch);
    let rtc = Rtc::new(peripherals.RTC_TIMER);
    // The RTC keeps time only across deep sleep; after other resets it starts from the
    // firmware build time, an estimate that SNTP or `TIME` replaces.
    let initial_rtc_us = rtc.current_time_us();
    let rtc_time_valid = reset_reason(Cpu::ProCpu) == Some(SocResetReason::CoreDeepSleep)
        && clock::is_rtc_time_valid(initial_rtc_us);
    #[cfg(feature = "wifi")]
    if rtc_time_valid {
        wireless::set_wall_clock(clock::unix_secs_from_rtc_micros(initial_rtc_us));
    }
    let mut clock_estimated = false;
    if !rtc_time_valid {
        if let Some(build_micros) = clock::rtc_micros_from_unix_secs(clock::BUILD_UNIX_SECS) {
            rtc.set_current_time_us(build_micros);
            clock_estimated = true;
            esp_println::println!(
                "Clock set to firmware build time {}; send TIME or connect Wi-Fi to correct it",
                clock::BUILD_UNIX_SECS
            );
        }
    }
    let time_valid = rtc_time_valid || clock_estimated;
    let buttons = Buttons {
        bank: ButtonBank::new(DEBOUNCE_MS, rtc.time_since_power_up().as_millis()),
        confirm: pull_up_input(peripherals.GPIO5),
        menu: pull_up_input(peripherals.GPIO2),
        exit: pull_up_input(peripherals.GPIO1),
        down: pull_up_input(peripherals.GPIO4),
        up: pull_up_input(peripherals.GPIO6),
    };

    let mut app = App {
        panel,
        display,
        rtc,
        uart: init_uart(peripherals.UART0, peripherals.GPIO44, peripherals.GPIO43),
        console: LineBuffer::new(),
        buttons,
        training,
        scratch,
        rng: Rng::new(),
        time_valid,
        clock_estimated,
        last_second: None,
        training_error: None,
        electricity_price: None,
        redraw: true,
        #[cfg(feature = "wifi")]
        wifi: wireless::Provisioning::new(wifi_available),
        #[cfg(feature = "wifi")]
        record_sync: mainichikatarenshu::supabase::RecordSync::default(),
    };
    #[cfg(feature = "wifi")]
    app.connect_saved_wifi();

    esp_println::println!("Commands: TIME <UTC unix seconds>, LIST, ACK <YYYY-MM-DD>");
    #[cfg(feature = "wifi")]
    esp_println::println!("Wi-Fi: SSID <name>, PASSWORD <passphrase>, CONNECT");
    loop {
        app.poll_console();
        #[cfg(feature = "wifi")]
        app.poll_network();
        let confirm_pressed = app.poll_buttons();
        let now = app.update_training(confirm_pressed);
        if app.redraw {
            app.refresh_display(now);
            if let Some(secs) = app.rest_pause(now) {
                pause_ms(u32::try_from(secs * 1000).unwrap_or(u32::MAX)).await;
            }
        }
        pause_ms(POLL_INTERVAL_MS).await;
    }
}

/// Configure SPI and control pins for the e-paper panel.
fn init_panel(
    spi: SPI2<'static>,
    sck: impl OutputPin + 'static,
    mosi: impl OutputPin + 'static,
    cs: impl OutputPin + 'static,
    dc: impl OutputPin + 'static,
    rst: impl OutputPin + 'static,
    busy: impl InputPin + 'static,
) -> Panel {
    let spi = Spi::new(spi, SpiConfig::default().with_frequency(Rate::from_mhz(4)))
        .expect("SPI configuration failed")
        .with_sck(sck)
        .with_mosi(mosi);
    let cs = Output::new(cs, Level::High, OutputConfig::default());
    let dc = Output::new(dc, Level::Low, OutputConfig::default());
    let rst = Output::new(rst, Level::High, OutputConfig::default());
    let busy = Input::new(busy, InputConfig::default());
    let spi = ExclusiveDevice::new(spi, cs, Delay::new()).expect("SPI CS initialization failed");
    Ssd1680::new(spi, busy, dc, rst, Delay::new()).expect("Display initialization failed")
}

/// Show a configuration error and stop: there is nothing to select from.
fn halt_unconfigured(panel: &mut Panel, display: &mut Display2in9) -> ! {
    layout::clear(display).expect("Display clear failed");
    layout::draw_centered_message(display, "kata.yaml missing").expect("Display rendering failed");
    panel
        .display_frame(display.buffer())
        .expect("Display update failed");
    panic!("Configure kata.yaml and rebuild the firmware");
}

/// Locate the `training` partition and load the persisted training state.
fn load_training(flash: FLASH<'static>, scratch: &mut [u8]) -> Training {
    let mut flash = FlashStorage::new(flash);
    flash
        .read(PARTITION_TABLE_OFFSET, scratch)
        .expect("Cannot read partition table");
    let range = find_training_partition(scratch, flash.capacity() as u32)
        .expect("Invalid or missing training partition");
    let store =
        StateStore::new(BlockingAsync::new(flash), range).expect("Invalid training storage range");
    block_on(PersistentTraining::load(store, scratch)).expect("Cannot load training data")
}

/// UART0 console on the board's USB serial bridge.
fn init_uart(
    uart: UART0<'static>,
    rx: impl InputPin + 'static,
    tx: impl OutputPin + 'static,
) -> Uart<'static, Blocking> {
    Uart::new(uart, UartConfig::default())
        .expect("UART configuration failed")
        .with_rx(rx)
        .with_tx(tx)
}

/// Active-low button input with the internal pull-up enabled.
fn pull_up_input(pin: impl InputPin + 'static) -> Input<'static> {
    Input::new(pin, InputConfig::default().with_pull(Pull::Up))
}

#[cfg(not(feature = "wifi"))]
async fn pause_ms(ms: u32) {
    Delay::new().delay_millis(ms);
}

#[cfg(feature = "wifi")]
async fn pause_ms(ms: u32) {
    embassy_time::Timer::after_millis(ms.into()).await;
}

/// Unwrap a training transaction, stopping on storage failures.
///
/// After a failed flash write the stored state is ambiguous, so continuing
/// could silently lose or duplicate records.
fn check_storage<T>(result: TrainingResult<T>, action: &str) -> Result<T, TrainingError> {
    match result {
        Ok(value) => Ok(value),
        Err(TransactionError::Training(error)) => Err(error),
        Err(TransactionError::Storage(error)) => panic!("{action} storage failed: {error:?}"),
        Err(TransactionError::RequiresReload) => panic!("Training storage requires reload"),
    }
}

/// The five CrowPanel buttons and their debouncers.
struct Buttons {
    bank: ButtonBank,
    confirm: Input<'static>,
    menu: Input<'static>,
    exit: Input<'static>,
    down: Input<'static>,
    up: Input<'static>,
}

/// Peripherals and state of the main loop.
struct App {
    panel: Panel,
    display: Display2in9,
    rtc: Rtc<'static>,
    uart: Uart<'static, Blocking>,
    console: LineBuffer<96>,
    buttons: Buttons,
    training: Training,
    /// Scratch buffer for flash operations.
    scratch: [u8; 4096],
    rng: Rng,
    /// Whether the RTC holds a usable time.
    time_valid: bool,
    /// Whether that time is only the firmware build time, not set by `TIME` or SNTP.
    clock_estimated: bool,
    /// The RTC second at which training was last advanced.
    last_second: Option<u64>,
    /// The error currently pausing the training workflow.
    training_error: Option<TrainingError>,
    electricity_price: Option<String<16>>,
    /// Whether the panel must be refreshed.
    redraw: bool,
    #[cfg(feature = "wifi")]
    wifi: wireless::Provisioning,
    /// Coordinates pending-record uploads with the network task.
    #[cfg(feature = "wifi")]
    record_sync: mainichikatarenshu::supabase::RecordSync,
}

impl App {
    /// Read pending serial bytes and execute completed command lines.
    fn poll_console(&mut self) {
        while self.uart.read_ready() {
            let mut byte = [0u8; 1];
            self.uart.read(&mut byte).expect("UART read failed");
            match self.console.push(byte[0]) {
                None => {}
                Some(Ok(line)) => self.execute_command(&line),
                Some(Err(LineError::TooLong)) => esp_println::println!("Command too long"),
                Some(Err(LineError::NotAscii)) => esp_println::println!("Command must be ASCII"),
            }
        }
    }

    fn execute_command(&mut self, line: &str) {
        match parse_command(line) {
            Ok(Command::SetTime(rtc_micros)) => {
                self.set_clock(rtc_micros);
                esp_println::println!("Clock set");
            }
            Ok(Command::List) => self.print_pending_records(),
            Ok(Command::Acknowledge(date)) => self.acknowledge(date),
            #[cfg(feature = "wifi")]
            Ok(Command::Ssid(ssid)) => self.wifi.stage_ssid(ssid),
            #[cfg(feature = "wifi")]
            Ok(Command::Password(password)) => self.wifi.stage_password(password),
            #[cfg(feature = "wifi")]
            Ok(Command::Connect) => self.connect_staged_wifi(),
            #[cfg(not(feature = "wifi"))]
            Ok(Command::Ssid(_) | Command::Password(_) | Command::Connect) => {
                esp_println::println!("Unknown command")
            }
            Err(CommandError::InvalidTime) => {
                esp_println::println!("Invalid TIME; provide UTC unix seconds since 2000")
            }
            Err(CommandError::InvalidDate) => esp_println::println!("Invalid ACK date"),
            Err(CommandError::Unknown) => esp_println::println!("Unknown command"),
        }
    }

    fn set_clock(&mut self, rtc_micros: u64) {
        self.rtc.set_current_time_us(rtc_micros);
        #[cfg(feature = "wifi")]
        wireless::set_wall_clock(clock::unix_secs_from_rtc_micros(rtc_micros));
        self.time_valid = true;
        self.clock_estimated = false;
        self.last_second = None;
        self.redraw = true;
    }

    fn print_pending_records(&self) {
        let pending = self.training.state().pending();
        esp_println::println!("Pending records: {}", pending.len());
        for record in pending {
            esp_println::println!(
                "{} | {:?} | confirmed: {:?}",
                record.date,
                record.kata,
                record.confirmed_at
            );
        }
    }

    fn acknowledge(&mut self, date: NaiveDate) {
        let result = block_on(self.training.acknowledge(&mut self.scratch, date));
        match check_storage(result, "ACK") {
            Ok(record) => {
                esp_println::println!("Record {} acknowledged", record.date);
                self.last_second = None;
            }
            Err(error) => esp_println::println!("ACK failed: {error:?}"),
        }
    }

    /// Sample all buttons; returns whether CONFIRM was pressed.
    fn poll_buttons(&mut self) -> bool {
        let buttons = &mut self.buttons;
        let levels = [
            (ButtonId::Confirm, buttons.confirm.is_high()),
            (ButtonId::Menu, buttons.menu.is_high()),
            (ButtonId::Exit, buttons.exit.is_high()),
            (ButtonId::Down, buttons.down.is_high()),
            (ButtonId::Up, buttons.up.is_high()),
        ];
        let now_ms = self.rtc.time_since_power_up().as_millis();
        let pressed = buttons
            .bank
            .pressed(levels, now_ms)
            .expect("Monotonic clock moved backwards");
        for id in &pressed {
            esp_println::println!("{id:?} button pressed");
            if matches!(id, ButtonId::Menu | ButtonId::Exit) {
                self.redraw = true;
            }
        }
        pressed.contains(&ButtonId::Confirm)
    }

    /// Advance the schedule once per second and handle confirmation.
    ///
    /// Returns the current time, or `None` while the clock is not set.
    fn update_training(&mut self, confirm_pressed: bool) -> Option<DateTime<Utc>> {
        if !self.time_valid {
            return None;
        }
        let rtc_micros = self.rtc.current_time_us();
        let now = clock::utc_from_rtc_micros(rtc_micros).expect("RTC timestamp out of range");
        let second = clock::unix_secs_from_rtc_micros(rtc_micros);
        if self.last_second != Some(second) {
            self.advance_training(now);
            self.last_second = Some(second);
        }
        if confirm_pressed && self.training_error.is_none() {
            self.confirm_training(now);
        }
        Some(now)
    }

    fn advance_training(&mut self, now: DateTime<Utc>) {
        let rng = &self.rng;
        let kata = choose_kata(KATA, || rng.random()).expect("KATA is not empty");
        let result = block_on(self.training.advance(&mut self.scratch, now, Some(kata)));
        match check_storage(result, "Training") {
            Ok(change) => {
                if self.training_error.take().is_some() || change != Change::None {
                    self.redraw = true;
                }
            }
            Err(error) => {
                if self.training_error != Some(error) {
                    esp_println::println!("Cannot advance training: {error:?}");
                    self.training_error = Some(error);
                    self.redraw = true;
                }
            }
        }
    }

    fn confirm_training(&mut self, now: DateTime<Utc>) {
        let result = block_on(self.training.confirm(&mut self.scratch, now));
        match check_storage(result, "Confirmation") {
            Ok(Change::Confirmed) => self.redraw = true,
            Ok(_) => {}
            Err(error) => esp_println::println!("Confirmation ignored: {error:?}"),
        }
    }

    /// Render the current screen, update the panel and put it to sleep.
    fn refresh_display(&mut self, now: Option<DateTime<Utc>>) {
        let screen = select_screen(
            now,
            self.training_error,
            self.training.state().active(),
            self.electricity_price.as_deref(),
        );
        draw_screen(&mut self.display, &screen).expect("Display rendering failed");
        self.panel
            .display_frame(self.display.buffer())
            .expect("Display update failed");
        let _ = self.panel.sleep();
        self.redraw = false;
    }

    /// Seconds to pause after a redraw while resting before 07:00, if idle.
    fn rest_pause(&self, now: Option<DateTime<Utc>>) -> Option<u64> {
        let now = now?;
        if self.training.state().active().is_some()
            || self.training_error.is_some()
            || self.uart.read_ready()
        {
            return None;
        }
        let secs = seconds_until_next_transition(now)?;
        esp_println::println!("Entering low-power sleep for {secs}s until 07:00");
        Some(secs.min(MAX_REST_SECS))
    }
}

#[cfg(feature = "wifi")]
impl App {
    /// Connect with credentials saved in flash, if any.
    fn connect_saved_wifi(&mut self) {
        if !self.wifi.is_available() {
            return;
        }
        match block_on(self.training.load_wifi_credentials(&mut self.scratch)) {
            Ok(Some(credentials)) => {
                esp_println::println!(
                    "Loaded saved Wi-Fi credentials from flash for SSID: {}",
                    credentials.ssid
                );
                self.wifi.submit(credentials);
            }
            Ok(None) => {}
            Err(error) => {
                esp_println::println!("Failed to load saved Wi-Fi credentials: {error:?}");
            }
        }
    }

    /// Validate, save and submit the credentials staged with `SSID` and `PASSWORD`.
    fn connect_staged_wifi(&mut self) {
        let Some(credentials) = self.wifi.staged_credentials() else {
            return;
        };
        match block_on(
            self.training
                .save_wifi_credentials(&mut self.scratch, &credentials),
        ) {
            Ok(()) => esp_println::println!("Saved Wi-Fi credentials to flash"),
            Err(error) => esp_println::println!(
                "Warning: Failed to save Wi-Fi credentials to flash: {error:?}"
            ),
        }
        self.wifi.submit(credentials);
    }

    /// Apply results from the network task and hand it the next record to sync.
    fn poll_network(&mut self) {
        if let Some(unix_secs) = wireless::TIME_SYNC.try_take() {
            if let Some(rtc_micros) = clock::rtc_micros_from_unix_secs(unix_secs) {
                self.set_clock(rtc_micros);
                esp_println::println!("Clock synchronized via SNTP: {unix_secs}");
            }
        }
        if let Ok(price) = wireless::PRICE_UPDATE.try_receive() {
            if self.electricity_price.as_ref() != Some(&price) {
                self.electricity_price = Some(price);
                self.redraw = true;
            }
        }
        if let Ok(outcome) = wireless::RECORD_SYNC_RESULT.try_receive() {
            if let Some(date) = self.record_sync.complete(outcome) {
                self.acknowledge(date);
            }
        }
        if self
            .record_sync
            .dispatch_next(
                mainichikatarenshu::supabase::CONFIGURED.is_some(),
                self.time_valid && !self.clock_estimated,
                self.training.state().pending(),
                |record| wireless::RECORD_SYNC_REQUEST.try_send(record),
            )
            .is_err()
        {
            esp_println::println!("Record sync request queue full; retrying");
        }
    }
}
