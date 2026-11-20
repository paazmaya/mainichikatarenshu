//! SSD1680 driver for the CrowPanel 2.9" panel.
//!
//! Every frame follows the operation flow of the SSD1680 datasheet (section 9.1):
//!
//! 1. Hardware reset and software reset (`0x12`), then wait for BUSY.
//! 2. Initialisation code: gate count (`0x01`), data entry mode (`0x11`), RAM window
//!    (`0x44`, `0x45`), border (`0x3C`), internal temperature sensor (`0x18`) and RAM
//!    address counters (`0x4E`, `0x4F`).
//! 3. Write the frame into black/white RAM (`0x24`).
//! 4. Refresh with Display Update Control 2 = [`Flag::DISPLAY_UPDATE_FULL`] (`0x22`) and
//!    Master Activation (`0x20`). This also loads the waveform LUT from OTP, so the driver
//!    never writes a LUT itself. Wait for BUSY.
//! 5. Fill the second RAM bank (`0x26`) with white so the next refresh starts from a
//!    known state.
//!
//! Re-initialising for every frame is what lets [`Ssd1680::display_frame`] work right
//! after [`Ssd1680::sleep`]: deep sleep can only be left by a hardware reset.

use display_interface::DisplayError;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::color::Color;
use crate::interface::DisplayInterface;
use crate::{cmd::Cmd, flag::Flag, FRAME_BYTES, HEIGHT, WIDTH};

/// Last gate line, sent as the gate count and as the end of the RAM Y window.
const LAST_ROW: u16 = HEIGHT - 1;
/// Last byte column, the end of the RAM X window.
const LAST_COLUMN_BYTE: u8 = (WIDTH / 8 - 1) as u8;

/// SSD1680 e-paper driver
///
/// Type parameters follow the argument order of [`Ssd1680::new`]:
///
/// - `SPI` - SPI device with chip select handled by the device
/// - `BUSY` - BUSY input, high while the controller is working
/// - `DC` - data/command select output
/// - `RST` - reset output, active low
/// - `DELAY` - delay provider
pub struct Ssd1680<SPI, BUSY, DC, RST, DELAY> {
    interface: DisplayInterface<SPI, BUSY, DC, RST, DELAY>,
}

impl<SPI, BUSY, DC, RST, DELAY> Ssd1680<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    /// Creates the driver and initialises the controller.
    ///
    /// Fails early if the panel does not respond, for example because of wiring problems.
    pub fn new(spi: SPI, busy: BUSY, dc: DC, rst: RST, delay: DELAY) -> Result<Self, DisplayError> {
        let mut driver = Self {
            interface: DisplayInterface::new(spi, busy, dc, rst, delay),
        };
        driver.init()?;
        Ok(driver)
    }

    /// Resets the controller and sends the initialisation code for this panel.
    ///
    /// This also wakes the controller from deep sleep.
    pub fn init(&mut self) -> Result<(), DisplayError> {
        log::debug!("Initialising SSD1680");
        let iface = &mut self.interface;

        iface.reset()?;
        iface.cmd(Cmd::SW_RESET)?;
        iface.wait_until_idle()?;

        iface.cmd_with_data(
            Cmd::DRIVER_CONTROL,
            &[LAST_ROW as u8, (LAST_ROW >> 8) as u8, 0x00],
        )?;
        iface.cmd_with_data(Cmd::DATA_ENTRY_MODE, &[Flag::DATA_ENTRY_INCRY_INCRX])?;
        iface.cmd_with_data(Cmd::SET_RAMX_START_END, &[0x00, LAST_COLUMN_BYTE])?;
        iface.cmd_with_data(
            Cmd::SET_RAMY_START_END,
            &[0x00, 0x00, LAST_ROW as u8, (LAST_ROW >> 8) as u8],
        )?;
        iface.cmd_with_data(Cmd::BORDER_WAVEFORM_CONTROL, &[Flag::BORDER_WAVEFORM_LUT1])?;
        iface.cmd_with_data(Cmd::TEMP_CONTROL, &[Flag::INTERNAL_TEMP_SENSOR])?;
        iface.cmd_with_data(Cmd::SET_RAMX_COUNTER, &[0x00])?;
        iface.cmd_with_data(Cmd::SET_RAMY_COUNTER, &[0x00, 0x00])?;
        iface.wait_until_idle()
    }

    /// Shows a frame on the panel and waits until the refresh has finished.
    ///
    /// `buffer` holds [`FRAME_BYTES`] bytes, as produced by
    /// [`Display2in9::buffer`](crate::Display::buffer). A full refresh takes a few seconds.
    pub fn display_frame(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        debug_assert_eq!(buffer.len(), FRAME_BYTES, "frame buffer has wrong size");

        self.init()?;
        self.interface.cmd_with_data(Cmd::WRITE_BW_DATA, buffer)?;
        self.refresh()?;
        self.clear_red_ram()
    }

    /// Puts the controller into deep sleep.
    ///
    /// The panel keeps showing the last frame without power. The next
    /// [`display_frame`](Self::display_frame) wakes the controller.
    pub fn sleep(&mut self) -> Result<(), DisplayError> {
        log::debug!("Putting SSD1680 into deep sleep");
        self.interface
            .cmd_with_data(Cmd::DEEP_SLEEP_MODE, &[Flag::DEEP_SLEEP_MODE_1])
    }

    /// Runs a full refresh from RAM and waits for it to complete.
    fn refresh(&mut self) -> Result<(), DisplayError> {
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[Flag::DISPLAY_UPDATE_FULL])?;
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.interface.wait_until_idle()
    }

    /// Fills the second RAM bank (`0x26`) with white.
    fn clear_red_ram(&mut self) -> Result<(), DisplayError> {
        self.interface.cmd(Cmd::WRITE_RED_DATA)?;
        self.interface
            .data_repeated(Color::White.byte_value(), FRAME_BYTES)?;
        self.interface.wait_until_idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use embedded_hal::digital::{self, ErrorType};
    use embedded_hal::spi::{self, Operation};
    use std::rc::Rc;
    use std::vec;
    use std::vec::Vec;

    #[derive(Debug, Clone, PartialEq)]
    enum Event {
        Cmd(u8),
        Data(Vec<u8>),
        /// Rising or falling edge on RST
        Reset(bool),
        /// BUSY was polled
        BusyPoll,
    }

    /// Shared state recording what the driver sends to the panel.
    #[derive(Default)]
    struct Bus {
        dc_high: bool,
        events: Vec<Event>,
        spi_writes: usize,
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
            bus.events.push(Event::Reset(high));
            Ok(())
        }
    }

    struct BusyPin(Shared);

    impl ErrorType for BusyPin {
        type Error = digital::ErrorKind;
    }

    impl InputPin for BusyPin {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            let mut bus = self.0.borrow_mut();
            if !bus.busy_stuck {
                bus.events.push(Event::BusyPoll);
            }
            Ok(bus.busy_stuck)
        }

        fn is_low(&mut self) -> Result<bool, Self::Error> {
            self.is_high().map(|high| !high)
        }
    }

    struct Delay;

    impl DelayNs for Delay {
        fn delay_ns(&mut self, _: u32) {}
    }

    type TestDriver = Ssd1680<Spi, BusyPin, DcPin, RstPin, Delay>;

    /// Driver connected to a fresh bus, with the events of `new()` discarded.
    fn setup() -> (Shared, TestDriver) {
        let bus = Shared::default();
        let driver = Ssd1680::new(
            Spi(bus.clone()),
            BusyPin(bus.clone()),
            DcPin(bus.clone()),
            RstPin(bus.clone()),
            Delay,
        )
        .unwrap();
        bus.borrow_mut().events.clear();
        (bus, driver)
    }

    fn init_sequence() -> Vec<Event> {
        use Event::*;
        vec![
            Reset(true),
            Reset(false),
            Reset(true),
            Cmd(0x12),
            BusyPoll,
            Cmd(0x01),
            Data(vec![0x27, 0x01, 0x00]),
            Cmd(0x11),
            Data(vec![0x03]),
            Cmd(0x44),
            Data(vec![0x00, 0x0F]),
            Cmd(0x45),
            Data(vec![0x00, 0x00, 0x27, 0x01]),
            Cmd(0x3C),
            Data(vec![0x01]),
            Cmd(0x18),
            Data(vec![0x80]),
            Cmd(0x4E),
            Data(vec![0x00]),
            Cmd(0x4F),
            Data(vec![0x00, 0x00]),
            BusyPoll,
        ]
    }

    #[test]
    fn new_runs_init_sequence() {
        let bus = Shared::default();
        Ssd1680::new(
            Spi(bus.clone()),
            BusyPin(bus.clone()),
            DcPin(bus.clone()),
            RstPin(bus.clone()),
            Delay,
        )
        .unwrap();

        assert_eq!(bus.borrow().events, init_sequence());
    }

    #[test]
    fn display_frame_sends_exact_panel_sequence() {
        use Event::*;
        let (bus, mut driver) = setup();
        let frame: Vec<u8> = (0..FRAME_BYTES).map(|i| i as u8).collect();
        driver.display_frame(&frame).unwrap();

        let mut expected = init_sequence();
        expected.extend([
            Cmd(0x24),
            Data(frame),
            Cmd(0x22),
            Data(vec![0xF4]),
            Cmd(0x20),
            BusyPoll,
            Cmd(0x26),
            Data(vec![0xFF; FRAME_BYTES]),
            BusyPoll,
        ]);
        assert_eq!(bus.borrow().events, expected);
    }

    #[test]
    fn sleep_enters_deep_sleep_mode_1() {
        let (bus, mut driver) = setup();
        driver.sleep().unwrap();

        assert_eq!(
            bus.borrow().events,
            [Event::Cmd(0x10), Event::Data(vec![0x01])]
        );
    }

    #[test]
    fn display_frame_after_sleep_resets_first() {
        let (bus, mut driver) = setup();
        driver.sleep().unwrap();
        bus.borrow_mut().events.clear();
        driver.display_frame(&[0; FRAME_BYTES]).unwrap();

        assert_eq!(bus.borrow().events[..3], init_sequence()[..3]);
    }

    #[test]
    fn red_ram_fill_is_split_into_chunks() {
        let (bus, mut driver) = setup();
        bus.borrow_mut().spi_writes = 0;
        driver.clear_red_ram().unwrap();

        // One command byte plus the frame in 32-byte chunks
        assert_eq!(bus.borrow().spi_writes, 1 + FRAME_BYTES.div_ceil(32));
    }

    #[test]
    fn spi_failures_are_reported_as_bus_errors() {
        // Fail on a command byte, a data payload and inside the repeated fill
        for fail_at in [0, 2, 25] {
            let (bus, mut driver) = setup();
            bus.borrow_mut().spi_writes = 0;
            bus.borrow_mut().fail_spi_at = Some(fail_at);
            assert!(
                matches!(
                    driver.display_frame(&[0; FRAME_BYTES]),
                    Err(DisplayError::BusWriteError)
                ),
                "fail_at={fail_at}"
            );
        }
    }

    #[test]
    fn pin_failures_are_reported() {
        let (bus, mut driver) = setup();
        bus.borrow_mut().fail_dc = true;
        assert!(matches!(driver.sleep(), Err(DisplayError::DCError)));

        let (bus, mut driver) = setup();
        bus.borrow_mut().fail_rst = true;
        assert!(matches!(driver.init(), Err(DisplayError::RSError)));
    }

    #[test]
    fn stuck_busy_pin_aborts_update() {
        let (bus, mut driver) = setup();
        bus.borrow_mut().busy_stuck = true;
        assert!(matches!(
            driver.display_frame(&[0; FRAME_BYTES]),
            Err(DisplayError::BusWriteError)
        ));
    }
}
