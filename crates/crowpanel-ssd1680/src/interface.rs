//! Low level SPI and GPIO access to the SSD1680 controller.
//!
//! - **DC** selects whether an SPI write is a command (low) or data (high).
//! - **RST** resets the controller when pulled low. A hardware reset is also the only way
//!   to leave deep sleep.
//! - **BUSY** stays high while the controller works. A full refresh keeps it high for one
//!   to three seconds, and no new commands may be sent before it goes low.

use display_interface::DisplayError;
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::SpiDevice,
};

/// How often BUSY is polled while waiting for the controller.
const BUSY_POLL_INTERVAL_MS: u32 = 10;
/// Upper bound for waiting on BUSY, well above a full refresh.
const BUSY_TIMEOUT_MS: u32 = 30_000;
/// Bytes written per SPI transfer when repeating a value.
const REPEAT_CHUNK_SIZE: usize = 32;

/// SPI and pin handles of the controller.
pub(crate) struct DisplayInterface<SPI, BUSY, DC, RST, DELAY> {
    spi: SPI,
    busy: BUSY,
    dc: DC,
    rst: RST,
    delay: DELAY,
}

impl<SPI, BUSY, DC, RST, DELAY> DisplayInterface<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    pub(crate) fn new(spi: SPI, busy: BUSY, dc: DC, rst: RST, delay: DELAY) -> Self {
        Self {
            spi,
            busy,
            dc,
            rst,
            delay,
        }
    }

    /// Sends a single command byte.
    pub(crate) fn cmd(&mut self, command: u8) -> Result<(), DisplayError> {
        self.dc.set_low().map_err(|_| DisplayError::DCError)?;
        self.spi.write(&[command]).map_err(|e| {
            log::error!("SPI write error for command 0x{command:02X}: {e:?}");
            DisplayError::BusWriteError
        })
    }

    /// Sends data bytes for the previous command.
    pub(crate) fn data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;
        self.spi
            .write(data)
            .map_err(|_| DisplayError::BusWriteError)
    }

    /// Sends a command followed by its data bytes.
    pub(crate) fn cmd_with_data(&mut self, command: u8, data: &[u8]) -> Result<(), DisplayError> {
        self.cmd(command)?;
        self.data(data)
    }

    /// Sends `value` as data `count` times, in chunks to avoid needing a full frame buffer.
    pub(crate) fn data_repeated(&mut self, value: u8, count: usize) -> Result<(), DisplayError> {
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;

        let chunk = [value; REPEAT_CHUNK_SIZE];
        let mut remaining = count;
        while remaining > 0 {
            let len = remaining.min(REPEAT_CHUNK_SIZE);
            self.spi
                .write(&chunk[..len])
                .map_err(|_| DisplayError::BusWriteError)?;
            remaining -= len;
        }
        Ok(())
    }

    /// Waits until BUSY goes low, failing after [`BUSY_TIMEOUT_MS`].
    pub(crate) fn wait_until_idle(&mut self) -> Result<(), DisplayError> {
        for _ in 0..BUSY_TIMEOUT_MS / BUSY_POLL_INTERVAL_MS {
            match self.busy.is_high() {
                Ok(false) => return Ok(()),
                Ok(true) => self.delay.delay_ms(BUSY_POLL_INTERVAL_MS),
                Err(_) => {
                    log::error!("Failed to read display BUSY pin");
                    return Err(DisplayError::BusWriteError);
                }
            }
        }
        log::error!("Timed out waiting for display BUSY pin");
        Err(DisplayError::BusWriteError)
    }

    /// Pulses RST low to reset the controller.
    pub(crate) fn reset(&mut self) -> Result<(), DisplayError> {
        self.set_rst(true)?;
        self.delay.delay_ms(20);
        self.set_rst(false)?;
        self.delay.delay_ms(2);
        self.set_rst(true)?;
        self.delay.delay_ms(20);
        Ok(())
    }

    fn set_rst(&mut self, high: bool) -> Result<(), DisplayError> {
        let result = if high {
            self.rst.set_high()
        } else {
            self.rst.set_low()
        };
        result.map_err(|_| DisplayError::RSError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use embedded_hal::digital::{ErrorKind, ErrorType};
    use embedded_hal::spi::Operation;

    struct MockSpi;

    impl embedded_hal::spi::ErrorType for MockSpi {
        type Error = Infallible;
    }

    impl SpiDevice for MockSpi {
        fn transaction(&mut self, _: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct MockOutput;

    impl ErrorType for MockOutput {
        type Error = Infallible;
    }

    impl OutputPin for MockOutput {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct MockBusy(Result<bool, ErrorKind>);

    impl ErrorType for MockBusy {
        type Error = ErrorKind;
    }

    impl InputPin for MockBusy {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            self.0
        }

        fn is_low(&mut self) -> Result<bool, Self::Error> {
            self.0.map(|high| !high)
        }
    }

    /// Records the total requested delay in milliseconds.
    #[derive(Default)]
    struct MockDelay(u32);

    impl DelayNs for MockDelay {
        fn delay_ns(&mut self, ns: u32) {
            self.0 += ns / 1_000_000;
        }
    }

    fn interface(
        busy: Result<bool, ErrorKind>,
    ) -> DisplayInterface<MockSpi, MockBusy, MockOutput, MockOutput, MockDelay> {
        DisplayInterface::new(
            MockSpi,
            MockBusy(busy),
            MockOutput,
            MockOutput,
            MockDelay::default(),
        )
    }

    #[test]
    fn wait_until_idle_returns_immediately_when_ready() {
        let mut interface = interface(Ok(false));
        assert!(interface.wait_until_idle().is_ok());
        assert_eq!(interface.delay.0, 0);
    }

    #[test]
    fn wait_until_idle_reports_pin_errors() {
        let mut interface = interface(Err(ErrorKind::Other));
        assert!(matches!(
            interface.wait_until_idle(),
            Err(DisplayError::BusWriteError)
        ));
    }

    #[test]
    fn wait_until_idle_times_out() {
        let mut interface = interface(Ok(true));
        assert!(matches!(
            interface.wait_until_idle(),
            Err(DisplayError::BusWriteError)
        ));
        assert_eq!(interface.delay.0, BUSY_TIMEOUT_MS);
    }
}
