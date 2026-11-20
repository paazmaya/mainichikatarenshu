use super::*;
use alloc::rc::Rc;
use core::cell::RefCell;
use embedded_hal::digital::{self, ErrorType};
use embedded_hal::spi::{self, Operation};

const FRAME_BYTES: usize = (WIDTH as usize / 8) * HEIGHT as usize;

#[derive(Debug, Clone, PartialEq)]
enum Event {
    Cmd(u8),
    Data(Vec<u8>),
}

/// Shared state recording what the driver sends to the panel.
#[derive(Default)]
struct Bus {
    dc_high: bool,
    events: Vec<Event>,
    spi_writes: usize,
    rst_levels: Vec<bool>,
    fail_spi_at: Option<usize>,
    fail_dc: bool,
    fail_rst: bool,
    busy_stuck: bool,
}

type Shared = Rc<RefCell<Bus>>;

struct Spi(Shared);

impl spi::ErrorType for Spi {
    type Error = spi::ErrorKind;
}

impl SpiDevice for Spi {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let mut bus = self.0.borrow_mut();
        for operation in operations {
            if let Operation::Write(bytes) = operation {
                if bus.fail_spi_at == Some(bus.spi_writes) {
                    return Err(spi::ErrorKind::Other);
                }
                bus.spi_writes += 1;
                if !bus.dc_high {
                    bus.events.push(Event::Cmd(bytes[0]));
                } else if let Some(Event::Data(data)) = bus.events.last_mut() {
                    // Consecutive data writes belong to the same command
                    data.extend_from_slice(bytes);
                } else {
                    bus.events.push(Event::Data(bytes.to_vec()));
                }
            }
        }
        Ok(())
    }
}

struct DcPin(Shared);

impl ErrorType for DcPin {
    type Error = digital::ErrorKind;
}

impl OutputPin for DcPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set(false)
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set(true)
    }
}

impl DcPin {
    fn set(&mut self, high: bool) -> Result<(), digital::ErrorKind> {
        let mut bus = self.0.borrow_mut();
        if bus.fail_dc {
            return Err(digital::ErrorKind::Other);
        }
        bus.dc_high = high;
        Ok(())
    }
}

struct RstPin(Shared);

impl ErrorType for RstPin {
    type Error = digital::ErrorKind;
}

impl OutputPin for RstPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set(false)
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set(true)
    }
}

impl RstPin {
    fn set(&mut self, high: bool) -> Result<(), digital::ErrorKind> {
        let mut bus = self.0.borrow_mut();
        if bus.fail_rst {
            return Err(digital::ErrorKind::Other);
        }
        bus.rst_levels.push(high);
        Ok(())
    }
}

struct BusyPin(Shared);

impl ErrorType for BusyPin {
    type Error = digital::ErrorKind;
}

impl InputPin for BusyPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.borrow().busy_stuck)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|high| !high)
    }
}

#[derive(Default)]
struct Delay;

impl DelayNs for Delay {
    fn delay_ns(&mut self, _: u32) {}
}

type TestDriver = Ssd1680<Spi, BusyPin, DcPin, RstPin, Delay>;

fn interface(bus: &Shared) -> DisplayInterface<Spi, BusyPin, DcPin, RstPin, Delay> {
    DisplayInterface::new(
        Spi(bus.clone()),
        BusyPin(bus.clone()),
        DcPin(bus.clone()),
        RstPin(bus.clone()),
        Delay,
    )
}

fn setup() -> (Shared, TestDriver) {
    let bus = Shared::default();
    let driver = Ssd1680::from_interface(interface(&bus));
    (bus, driver)
}

fn commands(bus: &Shared) -> Vec<u8> {
    bus.borrow()
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Cmd(cmd) => Some(*cmd),
            Event::Data(_) => None,
        })
        .collect()
}

/// Data payloads sent directly after each occurrence of `cmd`.
fn data_for(bus: &Shared, cmd: u8) -> Vec<Vec<u8>> {
    let events = &bus.borrow().events;
    events
        .windows(2)
        .filter_map(|pair| match pair {
            [Event::Cmd(c), Event::Data(data)] if *c == cmd => Some(data.clone()),
            _ => None,
        })
        .collect()
}

fn update_ctrl2_values(bus: &Shared) -> Vec<u8> {
    data_for(bus, Cmd::DISPLAY_UPDATE_CTRL2)
        .into_iter()
        .map(|data| data[0])
        .collect()
}

#[test]
fn new_resets_and_loads_full_update_lut() {
    let bus = Shared::default();
    let _driver = Ssd1680::new(
        Spi(bus.clone()),
        BusyPin(bus.clone()),
        DcPin(bus.clone()),
        RstPin(bus.clone()),
        Delay,
    )
    .unwrap();

    assert_eq!(bus.borrow().rst_levels, [true, false, true]);
    assert_eq!(commands(&bus)[0], Cmd::SW_RESET);
    assert_eq!(
        data_for(&bus, Cmd::WRITE_LUT_REGISTER),
        [TestDriver::LUT_FULL_UPDATE.to_vec()]
    );
    assert_eq!(
        data_for(&bus, Cmd::SET_RAMY_START_END).last().unwrap(),
        &[0x00, 0x00, 0x27, 0x01]
    );
}

#[test]
fn cpp_init_matches_arduino_sequence() {
    let (bus, mut driver) = setup();
    driver.cpp_init().unwrap();

    assert_eq!(
        commands(&bus),
        [0x12, 0x01, 0x11, 0x44, 0x45, 0x3C, 0x18, 0x4E, 0x4F]
    );
    assert_eq!(data_for(&bus, 0x01), [vec![0x27, 0x01, 0x00]]);
    assert_eq!(data_for(&bus, 0x45), [vec![0x00, 0x00, 0x27, 0x01]]);
}

#[test]
fn write_buffer_and_update_sends_buffer_and_clears_red_ram() {
    let (bus, mut driver) = setup();
    let buffer: Vec<u8> = (0..FRAME_BYTES).map(|i| i as u8).collect();
    driver.write_buffer_and_update(&buffer).unwrap();

    assert_eq!(data_for(&bus, Cmd::WRITE_BW_DATA), [buffer]);
    assert_eq!(update_ctrl2_values(&bus), [Flag::DISPLAY_UPDATE_FULL]);
    assert_eq!(
        data_for(&bus, Cmd::WRITE_RED_DATA),
        [vec![0xFF; FRAME_BYTES]]
    );
    // Panel is re-initialised before writing so it also wakes from deep sleep
    assert_eq!(commands(&bus)[0], Cmd::SW_RESET);
}

#[test]
fn fill_update_clear_selects_border_from_color() {
    for (color, border) in [
        (0x00, Flag::BORDER_WAVEFORM_BLACK),
        (0xFF, Flag::BORDER_WAVEFORM_WHITE),
    ] {
        let (bus, mut driver) = setup();
        driver.fill_update_clear(color).unwrap();

        assert_eq!(data_for(&bus, Cmd::BORDER_WAVEFORM_CONTROL), [vec![border]]);
        assert_eq!(
            data_for(&bus, Cmd::WRITE_BW_DATA),
            [vec![color; FRAME_BYTES]]
        );
        assert_eq!(update_ctrl2_values(&bus), [Flag::DISPLAY_UPDATE_FULL]);
    }
}

#[test]
fn update_frame_optionally_inverts_data() {
    let buffer = [0x0F, 0xA5, 0x00];

    let (bus, mut driver) = setup();
    driver.update_frame(&buffer).unwrap();
    driver.update_frame_with_inversion(&buffer, false).unwrap();
    driver.update_frame_with_inversion(&buffer, true).unwrap();

    assert_eq!(
        data_for(&bus, Cmd::WRITE_BW_DATA),
        [buffer.to_vec(), buffer.to_vec(), vec![0xF0, 0x5A, 0xFF]]
    );
    assert!(data_for(&bus, Cmd::SET_RAMX_START_END)
        .iter()
        .all(|window| window == &[0x00, 0x0F]));
}

#[test]
fn test_patterns_are_sent_normal_then_inverted() {
    let (bus, mut driver) = setup();
    driver.draw_test_pattern().unwrap();
    let frames = data_for(&bus, Cmd::WRITE_BW_DATA);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].len(), FRAME_BYTES);
    assert!(frames[0].iter().zip(&frames[1]).all(|(a, b)| *b == !*a));

    let (bus, mut driver) = setup();
    driver.draw_simple_test_pattern().unwrap();
    let frames = data_for(&bus, Cmd::WRITE_BW_DATA);
    let (top, bottom) = frames[0].split_at(FRAME_BYTES / 2);
    assert!(top.iter().all(|&b| b == 0x00) && bottom.iter().all(|&b| b == 0xFF));
    assert!(frames[1].iter().zip(&frames[0]).all(|(a, b)| *a == !*b));

    let (bus, mut driver) = setup();
    driver.white_and_black_test_pattern().unwrap();
    let frames = data_for(&bus, Cmd::WRITE_BW_DATA);
    let (top, bottom) = frames[0].split_at(FRAME_BYTES / 2);
    assert!(top.iter().all(|&b| b == 0xFF) && bottom.iter().all(|&b| b == 0x00));
    assert!(frames[1].iter().zip(&frames[0]).all(|(a, b)| *a == !*b));
    assert_eq!(bus.borrow().rst_levels.len(), 3);
}

#[test]
fn clear_frame_writes_white_and_refreshes() {
    let (bus, mut driver) = setup();
    driver.clear_frame().unwrap();

    let white = color::Color::White.get_byte_value();
    assert_eq!(
        data_for(&bus, Cmd::WRITE_BW_DATA),
        [vec![white; FRAME_BYTES]]
    );
    assert_eq!(
        data_for(&bus, Cmd::DISPLAY_UPDATE_CTRL1),
        [vec![Flag::DISPLAY_UPDATE_BW_RAM]]
    );
    assert_eq!(update_ctrl2_values(&bus), [Flag::DISPLAY_UPDATE_FULL]);
    assert_eq!(commands(&bus).last(), Some(&Cmd::NOP));
}

#[test]
fn sleep_and_wake_toggle_deep_sleep_mode() {
    let (bus, mut driver) = setup();
    driver.sleep().unwrap();
    driver.wake_up().unwrap();

    assert_eq!(
        data_for(&bus, Cmd::DEEP_SLEEP_MODE),
        [
            vec![Flag::DEEP_SLEEP_MODE_1],
            vec![Flag::DEEP_SLEEP_NORMAL_MODE]
        ]
    );
}

#[test]
fn update_sequences_use_expected_control_values() {
    type Sequence = fn(&mut TestDriver) -> Result<(), DisplayError>;
    let cases: [(Sequence, &[u8]); 6] = [
        (TestDriver::fast_update, &[0xB1, 0x91, 0xC7]),
        (
            TestDriver::arduino_fast_update,
            &[
                Flag::DISPLAY_UPDATE_FAST,
                Flag::DISPLAY_UPDATE_PARTIAL_1,
                Flag::DISPLAY_UPDATE_PARTIAL_2,
            ],
        ),
        (
            TestDriver::arduino_full_update,
            &[Flag::DISPLAY_UPDATE_FULL],
        ),
        (TestDriver::cpp_update, &[Flag::DISPLAY_UPDATE_FULL]),
        (TestDriver::direct_update_display, &[0xF4]),
        (TestDriver::display_frame, &[Flag::DISPLAY_UPDATE_FULL]),
    ];

    for (sequence, expected) in cases {
        let (bus, mut driver) = setup();
        sequence(&mut driver).unwrap();
        assert_eq!(update_ctrl2_values(&bus), expected);
        assert!(commands(&bus).contains(&Cmd::MASTER_ACTIVATE));
    }

    let (bus, mut driver) = setup();
    driver.fast_update().unwrap();
    assert_eq!(data_for(&bus, Cmd::TEMP_CONTROL_WRITE), [vec![0x64, 0x00]]);
}

#[test]
fn clear_sequences_fill_ram_with_white() {
    let (bus, mut driver) = setup();
    driver.factory_reset_clear().unwrap();
    assert_eq!(bus.borrow().rst_levels.len(), 9);
    assert_eq!(
        data_for(&bus, Cmd::WRITE_BW_DATA),
        [vec![0xFF; FRAME_BYTES]]
    );
    assert_eq!(
        data_for(&bus, Cmd::AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN),
        [vec![0xFF]]
    );
    assert_eq!(update_ctrl2_values(&bus), [0xC7, 0xF7]);

    let (bus, mut driver) = setup();
    driver.direct_clear().unwrap();
    assert_eq!(bus.borrow().rst_levels.len(), 6);
    assert_eq!(
        data_for(&bus, Cmd::WRITE_BW_DATA),
        [vec![0xFF; FRAME_BYTES]]
    );
    assert_eq!(update_ctrl2_values(&bus), [Flag::DISPLAY_UPDATE_FULL]);
}

#[test]
fn init_2point9_inch_loads_minimal_lut() {
    let (bus, mut driver) = setup();
    driver.init_2point9_inch().unwrap();

    let luts = data_for(&bus, Cmd::WRITE_LUT_REGISTER);
    assert_eq!(luts.len(), 1);
    assert_eq!(luts[0].len(), 70);
    assert_eq!(&luts[0][..2], &[0x40, 0x00]);
    assert_eq!(data_for(&bus, 0x3A), [vec![0x1A]]);
    assert_eq!(bus.borrow().rst_levels.len(), 6);
}

#[test]
fn arduino_display_image_writes_image_in_chunks() {
    let (bus, mut driver) = setup();
    let image: Vec<u8> = (0..1200).map(|i| (i % 251) as u8).collect();
    driver.arduino_display_image(&image).unwrap();

    assert_eq!(data_for(&bus, Cmd::WRITE_BW_DATA), [image]);
    assert_eq!(update_ctrl2_values(&bus), [Flag::DISPLAY_UPDATE_FULL]);
    assert_eq!(commands(&bus).last(), Some(&Cmd::MASTER_ACTIVATE));
}

#[test]
fn direct_access_helpers_pass_through() {
    let (bus, mut driver) = setup();
    driver.direct_cmd(0xAB).unwrap();
    driver.direct_data(&[1, 2, 3]).unwrap();
    driver.wait_busy().unwrap();
    driver.interface_reset().unwrap();

    assert_eq!(
        bus.borrow().events,
        [Event::Cmd(0xAB), Event::Data(vec![1, 2, 3])]
    );
    assert_eq!(bus.borrow().rst_levels, [true, false, true]);
}

#[test]
fn data_x_times_sends_full_chunks_and_remainder() {
    let (bus, mut driver) = setup();
    driver.interface.data_x_times(0xAA, 40).unwrap();

    assert_eq!(bus.borrow().events, [Event::Data(vec![0xAA; 40])]);
    assert_eq!(bus.borrow().spi_writes, 2);
}

#[test]
fn spi_failures_are_reported_as_bus_errors() {
    // Fail on the command byte, the data payload, a full chunk and the remainder
    for (fail_at, repetitions) in [(0, 0), (1, 0), (1, 64), (2, 40)] {
        let (bus, mut driver) = setup();
        bus.borrow_mut().fail_spi_at = Some(fail_at);
        let result = if repetitions == 0 {
            driver
                .direct_cmd(0x01)
                .and_then(|_| driver.direct_data(&[0]))
        } else {
            driver
                .direct_cmd(0x01)
                .and_then(|_| driver.interface.data_x_times(0, repetitions))
        };
        assert!(
            matches!(result, Err(DisplayError::BusWriteError)),
            "fail_at={fail_at}"
        );
    }
}

#[test]
fn pin_failures_are_reported() {
    let (bus, mut driver) = setup();
    bus.borrow_mut().fail_dc = true;
    assert!(matches!(
        driver.direct_cmd(0x01),
        Err(DisplayError::DCError)
    ));
    assert!(matches!(
        driver.direct_data(&[0]),
        Err(DisplayError::DCError)
    ));
    assert!(matches!(
        driver.interface.data_x_times(0, 1),
        Err(DisplayError::DCError)
    ));

    let (bus, mut driver) = setup();
    bus.borrow_mut().fail_rst = true;
    assert!(matches!(driver.cpp_init(), Err(DisplayError::RSError)));
}

#[test]
fn stuck_busy_pin_aborts_update() {
    let (bus, mut driver) = setup();
    bus.borrow_mut().busy_stuck = true;
    assert!(matches!(
        driver.cpp_update(),
        Err(DisplayError::BusWriteError)
    ));
}
