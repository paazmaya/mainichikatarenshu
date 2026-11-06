use super::*;
use core::convert::Infallible;
use embedded_hal::digital::{ErrorKind, ErrorType};
use embedded_hal::spi::{Operation, SpiDevice};

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
fn busy_wait_succeeds_when_ready() {
    let mut interface = interface(Ok(false));
    assert!(interface.wait_busy_low().is_ok());
    assert_eq!(interface.delay.0, 0);
}

#[test]
fn busy_wait_reports_pin_errors() {
    let mut interface = interface(Err(ErrorKind::Other));
    assert!(matches!(
        interface.wait_busy_low(),
        Err(DisplayError::BusWriteError)
    ));
}

#[test]
fn busy_wait_times_out() {
    let mut interface = interface(Ok(true));
    assert!(matches!(
        interface.wait_busy_low(),
        Err(DisplayError::BusWriteError)
    ));
    assert_eq!(interface.delay.0, 30_000);
}
